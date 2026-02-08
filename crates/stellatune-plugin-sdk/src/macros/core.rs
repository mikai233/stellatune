#[macro_export]
macro_rules! host_log {
    ($lvl:expr, $($arg:tt)*) => {{
        $crate::host_log($lvl, &format!($($arg)*));
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __st_opt_get_interface {
    () => {
        None
    };
    ($f:path) => {
        Some($f)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __st_opt_info_json {
    () => {
        None::<&str>
    };
    ($v:expr) => {
        Some($v)
    };
}
