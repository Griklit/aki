use aki::Anime;

pub fn reset_info() -> anyhow::Result<()> {
    for mut anime in Anime::iter_all() {
        let Some(Ok(bangumi)) = anime.bangumi_subject() else {
            continue;
        };
        let new = Anime::from(bangumi);
        anime.internationalizations = new.internationalizations;
        anime.save_as_meta_id_force().unwrap();
    }
    Ok(())
}

fn main() {
    reset_info().unwrap();
}
