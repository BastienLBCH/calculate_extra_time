cargo build --release
cargo build --target=x86_64-pc-windows-gnu --release

cp target/release/calculate_extra_time bin/calculate_extra_time__apple_silicon
cp target/x86_64-pc-windows-gnu/release/calculate_extra_time.exe bin/calculate_extra_time__windows_x86_64.exe
