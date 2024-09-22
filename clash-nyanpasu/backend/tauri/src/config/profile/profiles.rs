use super::{item::ProfileItem, item_type::ProfileUid};
use crate::utils::{dirs, help};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_yaml::Mapping;
use std::{borrow::Borrow, fs, io::Write};
use tracing_attributes::instrument;

/// Define the `profiles.yaml` schema
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct IProfiles {
    /// same as PrfConfig.current
    pub current: Option<ProfileUid>,

    /// same as PrfConfig.chain
    pub chain: Option<Vec<ProfileUid>>,

    /// record valid fields for clash
    pub valid: Option<Vec<ProfileUid>>,

    /// profile list
    pub items: Option<Vec<ProfileItem>>,
}

macro_rules! patch {
    ($lv: expr, $rv: expr, $key: tt) => {
        if ($rv.$key).is_some() {
            $lv.$key = $rv.$key;
        }
    };
}

impl IProfiles {
    pub fn new() -> Self {
        match dirs::profiles_path().and_then(|path| help::read_yaml::<Self>(&path)) {
            Ok(mut profiles) => {
                if profiles.items.is_none() {
                    profiles.items = Some(vec![]);
                }
                // compatible with the old old old version
                if let Some(items) = profiles.items.as_mut() {
                    for item in items.iter_mut() {
                        if item.uid.is_none() {
                            item.uid = Some(help::get_uid("d"));
                        }
                    }
                }
                profiles
            }
            Err(err) => {
                log::error!(target: "app", "{err}");
                Self::template()
            }
        }
    }

    pub fn template() -> Self {
        Self {
            valid: Some(vec![
                "dns".into(),
                "unified-delay".into(),
                "tcp-concurrent".into(),
            ]),
            items: Some(vec![]),
            ..Self::default()
        }
    }

    pub fn save_file(&self) -> Result<()> {
        help::save_yaml(
            &dirs::profiles_path()?,
            self,
            Some("# Profiles Config for Clash Nyanpasu"),
        )
    }

    /// 只修改current，valid和chain
    pub fn patch_config(&mut self, patch: IProfiles) -> Result<()> {
        if self.items.is_none() {
            self.items = Some(vec![]);
        }

        if let Some(current) = patch.current {
            let items = self.items.as_ref().unwrap();
            let some_uid = Some(current);

            if items.iter().any(|e| e.uid == some_uid) {
                self.current = some_uid;
            }
        }

        if let Some(chain) = patch.chain {
            self.chain = Some(chain);
        }

        if let Some(valid) = patch.valid {
            self.valid = Some(valid);
        }

        Ok(())
    }

    pub fn get_current(&self) -> Option<String> {
        self.current.clone()
    }

    /// get items ref
    pub fn get_items(&self) -> Option<&Vec<ProfileItem>> {
        self.items.as_ref()
    }

    /// find the item by the uid
    pub fn get_item(&self, uid: &String) -> Result<&ProfileItem> {
        if let Some(items) = self.items.as_ref() {
            let some_uid = Some(uid.clone());

            for each in items.iter() {
                if each.uid == some_uid {
                    return Ok(each);
                }
            }
        }

        bail!("failed to get the profile item \"uid:{uid}\"");
    }

    /// append new item
    /// if the file_data is some
    /// then should save the data to file
    pub fn append_item(&mut self, mut item: ProfileItem) -> Result<()> {
        if item.uid.is_none() {
            bail!("the uid should not be null");
        }

        // save the file data
        // move the field value after save
        if let Some(file_data) = item.file_data.take() {
            if item.file.is_none() {
                bail!("the file should not be null");
            }

            let file = item.file.clone().unwrap();
            let path = dirs::app_profiles_dir()?.join(&file);

            fs::File::create(path)
                .with_context(|| format!("failed to create file \"{}\"", file))?
                .write(file_data.as_bytes())
                .with_context(|| format!("failed to write to file \"{}\"", file))?;
        }

        if self.items.is_none() {
            self.items = Some(vec![]);
        }

        if let Some(items) = self.items.as_mut() {
            items.push(item)
        }
        self.save_file()
    }

    /// reorder items
    pub fn reorder(&mut self, active_id: String, over_id: String) -> Result<()> {
        let mut items = self.items.take().unwrap_or_default();
        let mut old_index = None;
        let mut new_index = None;

        for (i, item) in items.iter().enumerate() {
            if item.uid == Some(active_id.clone()) {
                old_index = Some(i);
            }
            if item.uid == Some(over_id.clone()) {
                new_index = Some(i);
            }
        }

        if old_index.is_none() || new_index.is_none() {
            return Ok(());
        }
        let item = items.remove(old_index.unwrap());
        items.insert(new_index.unwrap(), item);
        self.items = Some(items);
        self.save_file()
    }

    /// reorder items with the full order list
    pub fn reorder_by_list<T: Borrow<String>>(&mut self, order: &[T]) -> Result<()> {
        let mut items = self.items.take().unwrap_or_default();
        let mut new_items = vec![];

        for uid in order {
            if let Some(index) = items
                .iter()
                .position(|e| e.uid.as_ref() == Some(uid.borrow()))
            {
                new_items.push(items.remove(index));
            }
        }

        self.items = Some(new_items);
        self.save_file()
    }

    /// update the item value
    #[instrument]
    pub fn patch_item(&mut self, uid: String, item: ProfileItem) -> Result<()> {
        tracing::debug!("patch item: {uid} with {item:?}");
        let mut items = self.items.take().unwrap_or_default();

        for each in items.iter_mut() {
            if each.uid == Some(uid.clone()) {
                patch!(each, item, r#type);
                patch!(each, item, name);
                patch!(each, item, desc);
                patch!(each, item, file);
                patch!(each, item, url);
                patch!(each, item, selected);
                patch!(each, item, extra);
                patch!(each, item, updated);
                patch!(each, item, option);
                patch!(each, item, chains);
                tracing::debug!("patch item: {each:?}");
                self.items = Some(items);
                return self.save_file();
            }
        }

        self.items = Some(items);
        bail!("failed to find the profile item \"uid:{uid}\"")
    }

    /// be used to update the remote item
    /// only patch `updated` `extra` `file_data`
    pub fn update_item(&mut self, uid: String, mut item: ProfileItem) -> Result<()> {
        if self.items.is_none() {
            self.items = Some(vec![]);
        }

        // find the item
        let _ = self.get_item(&uid)?;

        if let Some(items) = self.items.as_mut() {
            let some_uid = Some(uid.clone());

            for each in items.iter_mut() {
                if each.uid == some_uid {
                    each.extra = item.extra;
                    each.updated = item.updated;

                    // save the file data
                    // move the field value after save
                    if let Some(file_data) = item.file_data.take() {
                        let file = each.file.take();
                        let file =
                            file.unwrap_or(item.file.take().unwrap_or(format!("{}.yaml", &uid)));

                        // the file must exists
                        each.file = Some(file.clone());

                        let path = dirs::app_profiles_dir()?.join(&file);

                        fs::File::create(path)
                            .with_context(|| format!("failed to create file \"{}\"", file))?
                            .write(file_data.as_bytes())
                            .with_context(|| format!("failed to write to file \"{}\"", file))?;
                    }

                    break;
                }
            }
        }

        self.save_file()
    }

    /// delete item
    /// if delete the current then return true
    pub fn delete_item(&mut self, uid: String) -> Result<bool> {
        let current = self.current.as_ref().unwrap_or(&uid);
        let current = current.clone();

        let mut items = self.items.take().unwrap_or_default();
        let mut index = None;

        // get the index
        for (i, item) in items.iter().enumerate() {
            if item.uid == Some(uid.clone()) {
                index = Some(i);
                break;
            }
        }

        if let Some(index) = index {
            if let Some(file) = items.remove(index).file {
                let _ = dirs::app_profiles_dir().map(|path| {
                    let path = path.join(file);
                    if path.exists() {
                        let _ = fs::remove_file(path);
                    }
                });
            }
        }

        // delete the original uid
        if current == uid {
            self.current = match !items.is_empty() {
                true => items[0].uid.clone(),
                false => None,
            };
        }

        self.items = Some(items);
        self.save_file()?;
        Ok(current == uid)
    }

    /// 获取current指向的配置内容
    pub fn current_mapping(&self) -> Result<Mapping> {
        match (self.current.as_ref(), self.items.as_ref()) {
            (Some(current), Some(items)) => {
                if let Some(item) = items.iter().find(|e| e.uid.as_ref() == Some(current)) {
                    let file_path = match item.file.as_ref() {
                        Some(file) => dirs::app_profiles_dir()?.join(file),
                        None => bail!("failed to get the file field"),
                    };
                    return help::read_merge_mapping(&file_path);
                }
                bail!("failed to find the current profile \"uid:{current}\"");
            }
            _ => Ok(Mapping::new()),
        }
    }
}
