macro_rules! chain_option {
    ($val:expr) => {if let Some(v) = $val {
        v
    } else {
        return None;
    }
    }
}