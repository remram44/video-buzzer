#[cfg(not(debug_assertions))]
use std::convert::Infallible;
#[cfg(not(debug_assertions))]
use warp::filters::any::any;
#[cfg(debug_assertions)]
use warp::fs::{file, File};
use warp::Filter;
#[cfg(debug_assertions)]
use warp::Rejection;

macro_rules! use_file {
    ($file:ident) => {
        use_file!($file, concat!(stringify!($file), ".html"));
    };
    ($name:ident, $path:expr) => {
        #[cfg(not(debug_assertions))]
        pub fn $name() -> impl Filter<Extract = (&'static [u8],), Error = Infallible> + Clone {
            any().map(|| include_bytes!(concat!("../", $path)) as &[u8])
        }

        #[cfg(debug_assertions)]
        pub fn $name() -> impl Filter<Extract = (File,), Error = Rejection> + Clone {
            file($path)
        }
    };
}

use_file!(video);
use_file!(join);
use_file!(buzzer);
use_file!(css_bootstrap, "css/bootstrap.min.css");
use_file!(css_custom, "css/custom.css");
