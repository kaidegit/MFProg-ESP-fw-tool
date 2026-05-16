use mfprog_esp_lib::json::{compress_entries, extract_flash_entries_from_json, read_flasher_args};
use slint::{Model, ModelRc, VecModel};
use std::path::PathBuf;
use std::rc::Rc;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    let ui_weak = ui.as_weak();
    ui.on_import_folder(move || {
        let ui = ui_weak.upgrade().unwrap();
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            let folder_str = folder.to_string_lossy().to_string();
            match read_flasher_args(&folder) {
                Ok(json) => {
                    match extract_flash_entries_from_json(&json, &folder) {
                        Ok(entries) => {
                            let rows: Vec<FlashRow> = entries
                                .into_iter()
                                .map(|e| FlashRow {
                                    enabled: true,
                                    file_path: e.file_path.to_string_lossy().to_string().into(),
                                    address: e.addr.into(),
                                })
                                .collect();
                            let model = Rc::new(VecModel::from(rows));
                            ui.set_rows(ModelRc::from(model));
                            ui.set_status_text(format!("导入成功: {}", folder_str).into());
                        }
                        Err(e) => {
                            ui.set_status_text(format!("Error 解析 JSON: {}", e).into());
                        }
                    }
                }
                Err(e) => {
                    ui.set_status_text(format!("Error 读取 flasher_args.json: {}", e).into());
                }
            }
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_add_row(move || {
        let ui = ui_weak.upgrade().unwrap();
        let rows = ui.get_rows();
        let model = rows
            .as_any()
            .downcast_ref::<VecModel<FlashRow>>()
            .unwrap();
        model.push(FlashRow {
            enabled: true,
            file_path: Default::default(),
            address: Default::default(),
        });
    });

    let ui_weak = ui.as_weak();
    ui.on_remove_selected(move || {
        let ui = ui_weak.upgrade().unwrap();
        let rows = ui.get_rows();
        let model = rows
            .as_any()
            .downcast_ref::<VecModel<FlashRow>>()
            .unwrap();
        let mut to_remove = Vec::new();
        for i in 0..model.row_count() {
            if model.row_data(i).unwrap().enabled {
                to_remove.push(i);
            }
        }
        for i in to_remove.into_iter().rev() {
            model.remove(i);
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_browse_file(move |index: i32| {
        let ui = ui_weak.upgrade().unwrap();
        if let Some(file) = rfd::FileDialog::new().pick_file() {
            let file_str = file.to_string_lossy().to_string();
            let rows = ui.get_rows();
            let model = rows
                .as_any()
                .downcast_ref::<VecModel<FlashRow>>()
                .unwrap();
            if let Some(mut row) = model.row_data(index as usize) {
                row.file_path = file_str.into();
                model.set_row_data(index as usize, row);
            }
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_browse_output(move || {
        let ui = ui_weak.upgrade().unwrap();
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            ui.set_output_folder(folder.to_string_lossy().to_string().into());
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_row_enabled_changed(move |index: i32, enabled: bool| {
        let ui = ui_weak.upgrade().unwrap();
        let rows = ui.get_rows();
        let model = rows
            .as_any()
            .downcast_ref::<VecModel<FlashRow>>()
            .unwrap();
        if let Some(mut row) = model.row_data(index as usize) {
            row.enabled = enabled;
            model.set_row_data(index as usize, row);
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_row_path_changed(move |index: i32, path: slint::SharedString| {
        let ui = ui_weak.upgrade().unwrap();
        let rows = ui.get_rows();
        let model = rows
            .as_any()
            .downcast_ref::<VecModel<FlashRow>>()
            .unwrap();
        if let Some(mut row) = model.row_data(index as usize) {
            row.file_path = path;
            model.set_row_data(index as usize, row);
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_row_addr_changed(move |index: i32, addr: slint::SharedString| {
        let ui = ui_weak.upgrade().unwrap();
        let rows = ui.get_rows();
        let model = rows
            .as_any()
            .downcast_ref::<VecModel<FlashRow>>()
            .unwrap();
        if let Some(mut row) = model.row_data(index as usize) {
            row.address = addr;
            model.set_row_data(index as usize, row);
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_start_compress(move || {
        let ui = ui_weak.upgrade().unwrap();
        let output = ui.get_output_folder();
        if output.is_empty() {
            ui.set_status_text("Error: 请先选择输出文件夹".into());
            return;
        }

        let rows = ui.get_rows();
        let model = rows
            .as_any()
            .downcast_ref::<VecModel<FlashRow>>()
            .unwrap();

        let mut entries = Vec::new();
        for i in 0..model.row_count() {
            let row = model.row_data(i).unwrap();
            if !row.enabled {
                continue;
            }
            let path = row.file_path.to_string();
            if path.is_empty() {
                continue;
            }
            entries.push(mfprog_esp_lib::FlashEntry {
                addr: row.address.to_string(),
                file_path: PathBuf::from(path),
            });
        }

        if entries.is_empty() {
            ui.set_status_text("Error: 没有有效的文件条目".into());
            return;
        }

        let output_path = PathBuf::from(output.to_string());
        match compress_entries(&entries, &output_path, None) {
            Ok(_) => {
                ui.set_status_text(format!("压缩完成: {}", output_path.display()).into());
            }
            Err(e) => {
                ui.set_status_text(format!("Error: {}", e).into());
            }
        }
    });

    ui.run()
}
