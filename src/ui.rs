mod implementation {
    use std::collections::{HashMap, HashSet, VecDeque};

    use serde::{Deserialize, Serialize};

    include!("ui/model.rs");
    include!("ui/runtime.rs");
    include!("ui/layout.rs");
    include!("ui/shared.rs");

    #[cfg(test)]
    mod tests {
        include!("ui/tests.rs");
    }
}

pub(crate) use implementation::*;
