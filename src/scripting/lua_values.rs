use std::collections::VecDeque;

use super::lua;

const MAX_SCRIPT_QUEUE_MESSAGES: usize = 64;
const MAX_SCRIPT_QUEUE_BYTES: usize = 256 * 1024;

pub(crate) fn queue_has_capacity(queue: &VecDeque<String>, additional_bytes: usize) -> bool {
    queue.len() < MAX_SCRIPT_QUEUE_MESSAGES
        && queue.iter().map(String::len).sum::<usize>() + additional_bytes <= MAX_SCRIPT_QUEUE_BYTES
}

pub(crate) fn ui_document_source(value: lua::Value) -> Result<String, String> {
    match value {
        lua::Value::String(value) => Ok(value.to_string_lossy()),
        lua::Value::Table(_) => {
            serde_json::to_string(&lua_value_to_json(value, 0)?).map_err(|error| error.to_string())
        }
        value => Err(format!(
            "ui:set_document expects a table or JSON string, received {}",
            value.type_name()
        )),
    }
}

pub(crate) fn lua_value_to_json(
    value: lua::Value,
    depth: usize,
) -> Result<serde_json::Value, String> {
    if depth > 32 {
        return Err("UI document table nesting exceeds 32 levels".to_owned());
    }
    match value {
        lua::Value::Nil => Ok(serde_json::Value::Null),
        lua::Value::Boolean(value) => Ok(serde_json::Value::Bool(value)),
        lua::Value::Integer(value) => Ok(serde_json::Value::Number(value.into())),
        lua::Value::Number(value) => serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| "UI document numbers must be finite".to_owned()),
        lua::Value::String(value) => Ok(serde_json::Value::String(value.to_string_lossy())),
        lua::Value::Table(table) if table.raw_len() > 0 => {
            let mut values = Vec::with_capacity(table.raw_len());
            for index in 1..=table.raw_len() {
                let value = table
                    .get::<lua::Value>(index)
                    .map_err(|error| error.to_string())?;
                values.push(lua_value_to_json(value, depth + 1)?);
            }
            Ok(serde_json::Value::Array(values))
        }
        lua::Value::Table(table) => {
            let mut values = serde_json::Map::new();
            for pair in table.pairs::<lua::Value, lua::Value>() {
                let (key, value) = pair.map_err(|error| error.to_string())?;
                let lua::Value::String(key) = key else {
                    return Err("UI document object keys must be strings".to_owned());
                };
                values.insert(key.to_string_lossy(), lua_value_to_json(value, depth + 1)?);
            }
            Ok(serde_json::Value::Object(values))
        }
        value => Err(format!(
            "UI documents cannot contain {} values",
            value.type_name()
        )),
    }
}

pub(crate) fn json_to_lua(
    lua: &lua::Lua,
    value: &serde_json::Value,
    depth: usize,
) -> lua::Result<lua::Value> {
    if depth > 32 {
        return Err(lua::Error::runtime(
            "network message nesting exceeds 32 levels",
        ));
    }
    match value {
        serde_json::Value::Null => Ok(lua::Value::Nil),
        serde_json::Value::Bool(value) => Ok(lua::Value::Boolean(*value)),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(lua::Value::Integer)
            .or_else(|| value.as_f64().map(lua::Value::Number))
            .ok_or_else(|| lua::Error::runtime("network message number is not finite")),
        serde_json::Value::String(value) => {
            #[cfg(not(target_arch = "wasm32"))]
            let value = lua.create_string(value)?;
            #[cfg(target_arch = "wasm32")]
            let value = lua.create_string(value);
            Ok(lua::Value::String(value))
        }
        serde_json::Value::Array(values) => {
            let table = create_table(lua)?;
            for (index, value) in values.iter().enumerate() {
                table.set(index + 1, json_to_lua(lua, value, depth + 1)?)?;
            }
            Ok(lua::Value::Table(table))
        }
        serde_json::Value::Object(values) => {
            let table = create_table(lua)?;
            for (key, value) in values {
                table.set(key.as_str(), json_to_lua(lua, value, depth + 1)?)?;
            }
            Ok(lua::Value::Table(table))
        }
    }
}

pub(crate) fn create_table(lua: &lua::Lua) -> lua::Result<lua::Table> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        lua.create_table()
    }

    #[cfg(target_arch = "wasm32")]
    {
        Ok(lua.create_table())
    }
}
