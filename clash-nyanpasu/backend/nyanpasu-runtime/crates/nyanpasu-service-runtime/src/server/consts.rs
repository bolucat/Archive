use std::path::PathBuf;

#[derive(Debug)]
pub struct RuntimeInfos {
    pub service_data_dir: PathBuf,
    pub service_config_dir: PathBuf,
    pub nyanpasu_config_dir: PathBuf,
    pub nyanpasu_data_dir: PathBuf,
    pub nyanpasu_app_dir: PathBuf,
}
