#[macro_export]
macro_rules! host_log {
    ($lvl:expr, $($arg:tt)*) => {{
        $crate::host_log($lvl, &format!($($arg)*));
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __st_opt_info {
    () => {
        None::<$crate::__private::serde_json::Map<String, $crate::__private::serde_json::Value>>
    };
    ($v:expr) => {
        Some(
            match $crate::__private::serde_json::to_value($v)
                .expect("export_plugin! `info` must be serializable")
            {
                $crate::__private::serde_json::Value::Object(v) => v,
                _ => panic!("export_plugin! `info` must serialize to a JSON object"),
            },
        )
    };
}
