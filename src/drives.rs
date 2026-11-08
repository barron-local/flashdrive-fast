use sysinfo::Disks;

#[derive(Debug, Clone)]
pub struct DriveInfo {
    pub mount_point: String,
    pub name: String,
    pub total_space: u64,
    pub available_space: u64,
    pub is_removable: bool,
    pub fs_type: String,
}

pub fn get_available_drives() -> Vec<DriveInfo> {
    let disks = Disks::new_with_refreshed_list();
    let mut drive_list = Vec::new();

    for disk in disks.list() {
        let mount = disk.mount_point().to_string_lossy().to_string();
        let name = disk.name().to_string_lossy().to_string();
        let fs_type = disk.file_system().to_string_lossy().to_string();

        drive_list.push(DriveInfo {
            mount_point: mount,
            name: if name.is_empty() { "Local Disk".to_string() } else { name },
            total_space: disk.total_space(),
            available_space: disk.available_space(),
            is_removable: disk.is_removable(),
            fs_type,
        });
    }

    drive_list.sort_by(|a, b| {
        // Show removable flash drives first
        b.is_removable.cmp(&a.is_removable).then(a.mount_point.cmp(&b.mount_point))
    });

    drive_list
}
