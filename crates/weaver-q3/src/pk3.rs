use weaver_asset::loading::Filesystem;
use weaver_util::Result;

pub trait Pk3Filesystem {
    fn with_pk3s_from_dir(self, directory: impl AsRef<std::path::Path>) -> Result<Self>
    where
        Self: Sized;
}

impl Pk3Filesystem for Filesystem {
    fn with_pk3s_from_dir(mut self, directory: impl AsRef<std::path::Path>) -> Result<Self> {
        let directory = directory.as_ref();
        let mut pk3s = vec![];
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "pk3").unwrap_or(false) {
                pk3s.push(path);
            }
        }
        // Sort pk3s in reverse order so that the last pk3 is the first to be searched
        // (for example, pak1.pk3 should be searched before pak0.pk3)
        // this is to allow for overriding assets in earlier pk3s (mod support)
        pk3s.sort_unstable();
        for pk3 in pk3s.into_iter().rev() {
            self.add_archive(pk3)?;
        }
        Ok(self)
    }
}
