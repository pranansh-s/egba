pub mod arm;
pub mod thumb;

#[macro_export]
macro_rules! format_reg {
    ($instr:expr, $range:expr) => {
        format!("R{:02}", bit_r!($instr, $range))
    };
}

#[macro_export]
macro_rules! bit_check {
    ($instr:expr, $bit:expr, $true_case:expr, $false_case:expr) => {
        if $instr.bit($bit) { $true_case } else { $false_case }
    };
}