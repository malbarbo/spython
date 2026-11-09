//! The functions of `math` that CPython takes from the libm of the system,
//! from the `libm` crate. They can differ from the libm of glibc in the last
//! bit.

macro_rules! unary {
    ($($export:ident = $func:ident;)*) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "C" fn $export(x: f64) -> f64 {
                libm::$func(x)
            }
        )*
    };
}

unary! {
    f64_exp = exp;
    f64_exp2 = exp2;
    f64_expm1 = expm1;
    f64_log = log;
    f64_log2 = log2;
    f64_log10 = log10;
    f64_log1p = log1p;
    f64_sin = sin;
    f64_cos = cos;
    f64_tan = tan;
    f64_asin = asin;
    f64_acos = acos;
    f64_atan = atan;
    f64_sinh = sinh;
    f64_cosh = cosh;
    f64_tanh = tanh;
    f64_asinh = asinh;
    f64_acosh = acosh;
    f64_atanh = atanh;
    f64_cbrt = cbrt;
    f64_erf = erf;
    f64_erfc = erfc;
}

#[unsafe(no_mangle)]
pub extern "C" fn f64_atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}
