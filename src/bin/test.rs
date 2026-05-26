fn main() {
    let x = sys_locale::get_locales().collect::<Vec<_>>();
    dbg!(x);
}
