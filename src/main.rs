mod backup;
mod dns;
mod nm;
mod presets;

use backup::{auto_backup_if_missing, force_backup, restore_from_backup};
use dns::{benchmark_servers, flush_caches, BenchResult};
use nm::{
    apply_dns, apply_snapshot, check_nmcli, format_dns_summary, get_connection_dns, get_device_dns,
    list_connections, restore_auto_dns, Connection,
};
use presets::{
    add_custom_server, add_group, find_server, load_config, remove_group, remove_server, save_config,
    AppConfig, DnsServer,
};

use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use std::collections::HashMap;

const APP_ID: &str = "io.github.DnsJump";

fn main() {
    let app = RelmApp::new(APP_ID);
    app.run::<AppModel>(());
}

#[derive(Debug)]
enum Msg {
    Refresh,
    ConnectionsLoaded(Result<Vec<Connection>, String>),
    SelectConnection(u32),
    SelectServer(String),
    CurrentDnsUpdated(String),
    Apply,
    ApplyDone(Result<String, String>),
    RestoreAuto,
    RestoreAutoDone(Result<String, String>),
    Flush,
    FlushDone(Result<String, String>),
    Benchmark,
    BenchmarkDone(Vec<BenchResult>),
    ApplyFastest,
    BackupNow,
    BackupDone(Result<String, String>),
    RestoreBackup,
    RestoreBackupDone(Result<String, String>),
    ShowAbout,
    ShowPrefs,
    PrefsClose,
    PrefsSave { test_domain: String, use_ipv6: bool },
    ShowAddServer,
    AddServerClose,
    AddServerSave {
        name: String,
        ipv4: String,
        ipv6: String,
    },
    DeleteSelected,
    ShowAddGroup,
    AddGroupClose,
    AddGroupSave(String),
    DeleteGroup,
    SelectGroup(u32),
    Busy(bool),
}

struct AppModel {
    config: AppConfig,
    connections: Vec<Connection>,
    selected_conn_idx: usize,
    selected_server_id: Option<String>,
    selected_group_idx: usize,
    current_dns_label: String,
    latencies: HashMap<String, String>,
    busy: bool,
    status: String,
    toast_overlay: adw::ToastOverlay,
    apply_fastest_after_bench: bool,
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        #[root]
        adw::Window {
            set_title: Some("DNS Jump"),
            set_default_width: 440,
            set_default_height: 620,

            #[local_ref]
            toast_overlay -> adw::ToastOverlay {
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,

                    adw::HeaderBar {
                        set_show_title: true,

                        pack_end = &gtk::MenuButton {
                            set_icon_name: "open-menu-symbolic",
                            set_tooltip_text: Some("Menu"),
                            #[wrap(Some)]
                            set_popover = &gtk::Popover {
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 4,
                                    set_margin_all: 8,

                                    gtk::Button {
                                        set_label: "Preferences",
                                        set_has_frame: false,
                                        connect_clicked => Msg::ShowPrefs,
                                    },
                                    gtk::Button {
                                        set_label: "Backup current DNS",
                                        set_has_frame: false,
                                        connect_clicked => Msg::BackupNow,
                                    },
                                    gtk::Button {
                                        set_label: "Restore from backup",
                                        set_has_frame: false,
                                        connect_clicked => Msg::RestoreBackup,
                                    },
                                    gtk::Button {
                                        set_label: "Add custom DNS",
                                        set_has_frame: false,
                                        connect_clicked => Msg::ShowAddServer,
                                    },
                                    gtk::Button {
                                        set_label: "Delete selected custom DNS",
                                        set_has_frame: false,
                                        connect_clicked => Msg::DeleteSelected,
                                    },
                                    gtk::Button {
                                        set_label: "Add group",
                                        set_has_frame: false,
                                        connect_clicked => Msg::ShowAddGroup,
                                    },
                                    gtk::Button {
                                        set_label: "Delete group",
                                        set_has_frame: false,
                                        connect_clicked => Msg::DeleteGroup,
                                    },
                                    gtk::Separator {},
                                    gtk::Button {
                                        set_label: "About",
                                        set_has_frame: false,
                                        connect_clicked => Msg::ShowAbout,
                                    },
                                }
                            }
                        },

                        pack_start = &gtk::Button {
                            set_icon_name: "view-refresh-symbolic",
                            set_tooltip_text: Some("Refresh"),
                            connect_clicked => Msg::Refresh,
                        },
                    },

                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_propagate_natural_height: true,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 12,
                            set_margin_all: 16,

                            adw::PreferencesGroup {
                                set_title: "Connection",
                                set_description: Some("NetworkManager profile to update"),

                                #[name = "conn_dropdown"]
                                gtk::DropDown {
                                    connect_selected_notify[sender] => move |dd| {
                                        sender.input(Msg::SelectConnection(dd.selected()));
                                    },
                                },

                                adw::ActionRow {
                                    set_title: "Current DNS",
                                    #[watch]
                                    set_subtitle: &model.current_dns_label,
                                },
                            },

                            adw::PreferencesGroup {
                                set_title: "Group",

                                #[name = "group_dropdown"]
                                gtk::DropDown {
                                    connect_selected_notify[sender] => move |dd| {
                                        sender.input(Msg::SelectGroup(dd.selected()));
                                    },
                                },
                            },

                            adw::PreferencesGroup {
                                set_title: "DNS servers",
                                set_description: Some("Select a preset or custom server"),

                                #[name = "server_list"]
                                gtk::ListBox {
                                    add_css_class: "boxed-list",
                                    set_selection_mode: gtk::SelectionMode::None,
                                },
                            },

                            adw::PreferencesGroup {
                                set_title: "Actions",

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 8,
                                    set_margin_top: 8,
                                    set_margin_bottom: 8,
                                    set_margin_start: 12,
                                    set_margin_end: 12,

                                    gtk::Button {
                                        set_label: "Apply DNS",
                                        add_css_class: "suggested-action",
                                        #[watch]
                                        set_sensitive: !model.busy,
                                        connect_clicked => Msg::Apply,
                                    },

                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 8,
                                        set_homogeneous: true,

                                        gtk::Button {
                                            set_label: "Restore Auto",
                                            #[watch]
                                            set_sensitive: !model.busy,
                                            connect_clicked => Msg::RestoreAuto,
                                        },
                                        gtk::Button {
                                            set_label: "Flush Cache",
                                            #[watch]
                                            set_sensitive: !model.busy,
                                            connect_clicked => Msg::Flush,
                                        },
                                    },

                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 8,
                                        set_homogeneous: true,

                                        gtk::Button {
                                            set_label: "Benchmark",
                                            #[watch]
                                            set_sensitive: !model.busy,
                                            connect_clicked => Msg::Benchmark,
                                        },
                                        gtk::Button {
                                            set_label: "Apply Fastest",
                                            #[watch]
                                            set_sensitive: !model.busy,
                                            connect_clicked => Msg::ApplyFastest,
                                        },
                                    },

                                    gtk::Label {
                                        #[watch]
                                        set_label: &model.status,
                                        set_wrap: true,
                                        set_xalign: 0.0,
                                        add_css_class: "dim-label",
                                    },
                                },
                            },
                        },
                    },
                }
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let config = load_config().unwrap_or_default();
        let toast_overlay = adw::ToastOverlay::new();

        let model = AppModel {
            selected_server_id: config.selected_server_id.clone(),
            config,
            connections: Vec::new(),
            selected_conn_idx: 0,
            selected_group_idx: 0,
            current_dns_label: "Loading…".into(),
            latencies: HashMap::new(),
            busy: false,
            status: String::new(),
            toast_overlay: toast_overlay.clone(),
            apply_fastest_after_bench: false,
        };

        let widgets = view_output!();

        sender.input(Msg::Refresh);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Msg::Refresh => {
                self.status = "Refreshing…".into();
                let last = self.config.last_connection.clone();
                std::thread::spawn(move || {
                    let result = (|| {
                        check_nmcli()?;
                        list_connections()
                    })()
                    .map_err(|e| e.to_string());
                    sender.input(Msg::ConnectionsLoaded(result));
                    let _ = last;
                });
            }

            Msg::ConnectionsLoaded(result) => match result {
                Ok(conns) => {
                    if conns.is_empty() {
                        self.status = "No NetworkManager connections found".into();
                        self.connections.clear();
                        return;
                    }
                    self.connections = conns;
                    // Prefer last connection or active
                    if let Some(last) = &self.config.last_connection {
                        if let Some(idx) = self.connections.iter().position(|c| &c.uuid == last) {
                            self.selected_conn_idx = idx;
                        } else if let Some(idx) = self.connections.iter().position(|c| c.active) {
                            self.selected_conn_idx = idx;
                        }
                    } else if let Some(idx) = self.connections.iter().position(|c| c.active) {
                        self.selected_conn_idx = idx;
                    }
                    self.rebuild_connection_dropdown(&sender);
                    self.rebuild_group_dropdown(&sender);
                    self.rebuild_server_list(&sender);
                    self.refresh_current_dns(&sender);
                    self.status = format!("{} connection(s)", self.connections.len());
                }
                Err(e) => {
                    self.status = e.clone();
                    self.toast(&e);
                }
            },

            Msg::SelectConnection(idx) => {
                let idx = idx as usize;
                if idx < self.connections.len() {
                    self.selected_conn_idx = idx;
                    self.config.last_connection =
                        Some(self.connections[idx].uuid.clone());
                    let _ = save_config(&self.config);
                    self.refresh_current_dns(&sender);
                }
            },

            Msg::SelectGroup(idx) => {
                self.selected_group_idx = idx as usize;
                self.rebuild_server_list(&sender);
            },

            Msg::SelectServer(id) => {
                self.selected_server_id = Some(id.clone());
                self.config.selected_server_id = Some(id);
                let _ = save_config(&self.config);
                self.rebuild_server_list(&sender);
            },

            Msg::CurrentDnsUpdated(label) => {
                self.current_dns_label = label;
            },

            Msg::Apply => {
                let Some(server) = self.selected_server() else {
                    self.toast("Select a DNS server first");
                    return;
                };
                let Some(conn) = self.selected_connection() else {
                    self.toast("Select a connection first");
                    return;
                };
                sender.input(Msg::Busy(true));
                let uuid = conn.uuid.clone();
                let name = conn.name.clone();
                let ipv4 = server.ipv4.clone();
                let ipv6 = server.ipv6.clone();
                let use_ipv6 = self.config.use_ipv6;
                std::thread::spawn(move || {
                    let result = (|| {
                        let snap = get_connection_dns(&uuid)?;
                        let _ = auto_backup_if_missing(&uuid, &name, &snap)?;
                        apply_dns(&uuid, &ipv4, &ipv6, use_ipv6)?;
                        let flush_msg = flush_caches().unwrap_or_else(|e| e.to_string());
                        Ok(format!("Applied DNS to {name}. {flush_msg}"))
                    })()
                    .map_err(|e: anyhow::Error| e.to_string());
                    sender.input(Msg::ApplyDone(result));
                });
            },

            Msg::ApplyDone(result) => {
                sender.input(Msg::Busy(false));
                match result {
                    Ok(msg) => {
                        self.toast(&msg);
                        self.status = msg;
                        self.refresh_current_dns(&sender);
                    }
                    Err(e) => {
                        self.toast(&e);
                        self.status = e;
                    }
                }
            },

            Msg::RestoreAuto => {
                let Some(conn) = self.selected_connection() else {
                    self.toast("Select a connection first");
                    return;
                };
                sender.input(Msg::Busy(true));
                let uuid = conn.uuid.clone();
                let name = conn.name.clone();
                std::thread::spawn(move || {
                    let result = (|| {
                        let snap = get_connection_dns(&uuid)?;
                        let _ = auto_backup_if_missing(&uuid, &name, &snap)?;
                        restore_auto_dns(&uuid)?;
                        let flush_msg = flush_caches().unwrap_or_else(|e| e.to_string());
                        Ok(format!("Restored automatic DNS on {name}. {flush_msg}"))
                    })()
                    .map_err(|e: anyhow::Error| e.to_string());
                    sender.input(Msg::RestoreAutoDone(result));
                });
            },

            Msg::RestoreAutoDone(result) => {
                sender.input(Msg::Busy(false));
                match result {
                    Ok(msg) => {
                        self.toast(&msg);
                        self.status = msg;
                        self.refresh_current_dns(&sender);
                    }
                    Err(e) => {
                        self.toast(&e);
                        self.status = e;
                    }
                }
            },

            Msg::Flush => {
                sender.input(Msg::Busy(true));
                std::thread::spawn(move || {
                    let result = flush_caches().map_err(|e| e.to_string());
                    sender.input(Msg::FlushDone(result));
                });
            },

            Msg::FlushDone(result) => {
                sender.input(Msg::Busy(false));
                match result {
                    Ok(msg) => {
                        self.toast(&msg);
                        self.status = msg;
                    }
                    Err(e) => {
                        self.toast(&e);
                        self.status = e;
                    }
                }
            },

            Msg::Benchmark => {
                sender.input(Msg::Busy(true));
                self.status = "Benchmarking…".into();
                let domain = self.config.test_domain.clone();
                let pairs: Vec<(String, String)> = self
                    .config
                    .servers
                    .iter()
                    .filter_map(|s| s.ipv4.first().map(|ip| (s.id.clone(), ip.clone())))
                    .collect();
                std::thread::spawn(move || {
                    let results = benchmark_servers(&pairs, &domain, 2500);
                    sender.input(Msg::BenchmarkDone(results));
                });
            },

            Msg::BenchmarkDone(results) => {
                sender.input(Msg::Busy(false));
                self.latencies.clear();
                for r in &results {
                    let label = match (r.latency_ms, &r.error) {
                        (Some(ms), _) => format!("{ms} ms"),
                        (None, Some(e)) => format!("fail: {e}"),
                        _ => "fail".into(),
                    };
                    self.latencies.insert(r.server_id.clone(), label);
                }
                if let Some(best) = results.iter().find(|r| r.latency_ms.is_some()) {
                    self.selected_server_id.replace(best.server_id.clone());
                    self.config.selected_server_id = Some(best.server_id.clone());
                    let _ = save_config(&self.config);
                    self.status = format!(
                        "Fastest: {} ({} ms)",
                        find_server(&self.config, &best.server_id)
                            .map(|s| s.name.as_str())
                            .unwrap_or("?"),
                        best.latency_ms.unwrap_or(0)
                    );
                } else {
                    self.status = "Benchmark finished — no successful lookups".into();
                }
                self.rebuild_server_list(&sender);
                self.toast("Benchmark complete");
                if self.apply_fastest_after_bench {
                    self.apply_fastest_after_bench = false;
                    if self.selected_server_id.is_some() {
                        sender.input(Msg::Apply);
                    }
                }
            },

            Msg::ApplyFastest => {
                if self.latencies.is_empty() {
                    self.apply_fastest_after_bench = true;
                    sender.input(Msg::Benchmark);
                    return;
                }
                // Find min latency among known
                let mut best: Option<(String, u64)> = None;
                for (id, label) in &self.latencies {
                    if let Some(ms_str) = label.strip_suffix(" ms") {
                        if let Ok(ms) = ms_str.parse::<u64>() {
                            let better = best.as_ref().map(|(_, b)| ms < *b).unwrap_or(true);
                            if better {
                                best = Some((id.clone(), ms));
                            }
                        }
                    }
                }
                if let Some((id, _)) = best {
                    self.selected_server_id = Some(id);
                    sender.input(Msg::Apply);
                } else {
                    self.toast("Run Benchmark first");
                }
            },

            Msg::BackupNow => {
                let Some(conn) = self.selected_connection() else {
                    self.toast("Select a connection first");
                    return;
                };
                let uuid = conn.uuid.clone();
                let name = conn.name.clone();
                std::thread::spawn(move || {
                    let result = (|| {
                        let snap = get_connection_dns(&uuid)?;
                        force_backup(&uuid, &name, &snap)?;
                        Ok(format!("Backed up DNS for {name}"))
                    })()
                    .map_err(|e: anyhow::Error| e.to_string());
                    sender.input(Msg::BackupDone(result));
                });
            },

            Msg::BackupDone(result) => match result {
                Ok(msg) => {
                    self.toast(&msg);
                    self.status = msg;
                }
                Err(e) => self.toast(&e),
            },

            Msg::RestoreBackup => {
                sender.input(Msg::Busy(true));
                std::thread::spawn(move || {
                    let result = (|| {
                        let (uuid, snap) = restore_from_backup()?;
                        apply_snapshot(&uuid, &snap)?;
                        let flush_msg = flush_caches().unwrap_or_else(|e| e.to_string());
                        Ok(format!("Restored backup. {flush_msg}"))
                    })()
                    .map_err(|e: anyhow::Error| e.to_string());
                    sender.input(Msg::RestoreBackupDone(result));
                });
            },

            Msg::RestoreBackupDone(result) => {
                sender.input(Msg::Busy(false));
                match result {
                    Ok(msg) => {
                        self.toast(&msg);
                        self.status = msg;
                        self.refresh_current_dns(&sender);
                    }
                    Err(e) => {
                        self.toast(&e);
                        self.status = e;
                    }
                }
            },

            Msg::ShowAbout => {
                let dialog = adw::AboutDialog::builder()
                    .application_name("DNS Jump")
                    .application_icon("io.github.DnsJump")
                    .developer_name("DNS Jump Contributors")
                    .version(env!("CARGO_PKG_VERSION"))
                    .comments("Switch DNS servers and flush cache on Linux via NetworkManager.")
                    .license_type(gtk::License::MitX11)
                    .website("https://github.com/DnsJump/dns-jump")
                    .build();
                dialog.present(Some(&root_window()));
            },

            Msg::ShowPrefs => {
                let domain = self.config.test_domain.clone();
                let use_ipv6 = self.config.use_ipv6;
                let dialog = adw::Dialog::new();
                dialog.set_title("Preferences");
                dialog.set_content_width(360);

                let box_ = gtk::Box::new(gtk::Orientation::Vertical, 12);
                box_.set_margin_all(16);

                let domain_entry = gtk::Entry::new();
                domain_entry.set_text(&domain);
                domain_entry.set_placeholder_text(Some("Test domain for benchmark"));

                let ipv6_switch = gtk::Switch::new();
                ipv6_switch.set_active(use_ipv6);
                ipv6_switch.set_halign(gtk::Align::Start);

                let domain_row = adw::ActionRow::new();
                domain_row.set_title("Benchmark domain");
                domain_row.add_suffix(&domain_entry);

                let ipv6_row = adw::ActionRow::new();
                ipv6_row.set_title("Apply IPv6 DNS");
                ipv6_row.add_suffix(&ipv6_switch);

                let group = adw::PreferencesGroup::new();
                group.add(&domain_row);
                group.add(&ipv6_row);

                let save_btn = gtk::Button::with_label("Save");
                save_btn.add_css_class("suggested-action");

                box_.append(&group);
                box_.append(&save_btn);
                dialog.set_child(Some(&box_));

                let sender2 = sender.clone();
                save_btn.connect_clicked(move |_| {
                    sender2.input(Msg::PrefsSave {
                        test_domain: domain_entry.text().to_string(),
                        use_ipv6: ipv6_switch.is_active(),
                    });
                    sender2.input(Msg::PrefsClose);
                });

                dialog.present(Some(&root_window()));
                // store not needed; ephemeral
                let _ = dialog;
            },

            Msg::PrefsClose => {}

            Msg::PrefsSave {
                test_domain,
                use_ipv6,
            } => {
                self.config.test_domain = test_domain.trim().to_string();
                if self.config.test_domain.is_empty() {
                    self.config.test_domain = "www.google.com".into();
                }
                self.config.use_ipv6 = use_ipv6;
                let _ = save_config(&self.config);
                self.toast("Preferences saved");
            },

            Msg::ShowAddServer => {
                let dialog = adw::Dialog::new();
                dialog.set_title("Add custom DNS");
                dialog.set_content_width(380);

                let box_ = gtk::Box::new(gtk::Orientation::Vertical, 10);
                box_.set_margin_all(16);

                let name = gtk::Entry::new();
                name.set_placeholder_text(Some("Name"));
                let ipv4 = gtk::Entry::new();
                ipv4.set_placeholder_text(Some("IPv4 (space-separated)"));
                let ipv6 = gtk::Entry::new();
                ipv6.set_placeholder_text(Some("IPv6 (optional, space-separated)"));

                let save = gtk::Button::with_label("Add");
                save.add_css_class("suggested-action");

                box_.append(&name);
                box_.append(&ipv4);
                box_.append(&ipv6);
                box_.append(&save);
                dialog.set_child(Some(&box_));

                let sender2 = sender.clone();
                save.connect_clicked(move |_| {
                    sender2.input(Msg::AddServerSave {
                        name: name.text().to_string(),
                        ipv4: ipv4.text().to_string(),
                        ipv6: ipv6.text().to_string(),
                    });
                    sender2.input(Msg::AddServerClose);
                });
                dialog.present(Some(&root_window()));
                let _ = dialog;
            },

            Msg::AddServerClose => {}

            Msg::AddServerSave { name, ipv4, ipv6 } => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    self.toast("Name is required");
                    return;
                }
                let ipv4: Vec<String> = ipv4
                    .split_whitespace()
                    .map(str::to_string)
                    .filter(|s| !s.is_empty())
                    .collect();
                if ipv4.is_empty() {
                    self.toast("At least one IPv4 address is required");
                    return;
                }
                let ipv6: Vec<String> = ipv6
                    .split_whitespace()
                    .map(str::to_string)
                    .filter(|s| !s.is_empty())
                    .collect();
                let id = add_custom_server(&mut self.config, name, ipv4, ipv6, None);
                self.selected_server_id = Some(id);
                let _ = save_config(&self.config);
                self.rebuild_server_list(&sender);
                self.toast("Custom DNS added");
            },

            Msg::DeleteSelected => {
                let Some(id) = self.selected_server_id.clone() else {
                    self.toast("Nothing selected");
                    return;
                };
                if remove_server(&mut self.config, &id) {
                    self.selected_server_id = None;
                    self.config.selected_server_id = None;
                    let _ = save_config(&self.config);
                    self.rebuild_server_list(&sender);
                    self.toast("Custom DNS removed");
                } else {
                    self.toast("Cannot delete built-in servers");
                }
            },

            Msg::ShowAddGroup => {
                let dialog = adw::Dialog::new();
                dialog.set_title("Add group");
                dialog.set_content_width(320);
                let box_ = gtk::Box::new(gtk::Orientation::Vertical, 10);
                box_.set_margin_all(16);
                let entry = gtk::Entry::new();
                entry.set_placeholder_text(Some("Group name"));
                let save = gtk::Button::with_label("Add");
                save.add_css_class("suggested-action");
                box_.append(&entry);
                box_.append(&save);
                dialog.set_child(Some(&box_));
                let sender2 = sender.clone();
                save.connect_clicked(move |_| {
                    sender2.input(Msg::AddGroupSave(entry.text().to_string()));
                    sender2.input(Msg::AddGroupClose);
                });
                dialog.present(Some(&root_window()));
                let _ = dialog;
            },

            Msg::AddGroupClose => {}

            Msg::AddGroupSave(name) => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    self.toast("Group name required");
                    return;
                }
                add_group(&mut self.config, name);
                let _ = save_config(&self.config);
                self.rebuild_group_dropdown(&sender);
                self.toast("Group added");
            },

            Msg::DeleteGroup => {
                if self.selected_group_idx >= self.config.groups.len() {
                    return;
                }
                let id = self.config.groups[self.selected_group_idx].id.clone();
                if remove_group(&mut self.config, &id) {
                    self.selected_group_idx = 0;
                    let _ = save_config(&self.config);
                    self.rebuild_group_dropdown(&sender);
                    self.rebuild_server_list(&sender);
                    self.toast("Group removed");
                } else {
                    self.toast("Cannot delete built-in groups");
                }
            },

            Msg::Busy(b) => {
                self.busy = b;
            },
        }
    }
}

impl AppModel {
    fn toast(&self, text: &str) {
        let toast = adw::Toast::new(text);
        toast.set_timeout(3);
        self.toast_overlay.add_toast(toast);
    }

    fn selected_connection(&self) -> Option<&Connection> {
        self.connections.get(self.selected_conn_idx)
    }

    fn selected_server(&self) -> Option<&DnsServer> {
        let id = self.selected_server_id.as_deref()?;
        find_server(&self.config, id)
    }

    fn refresh_current_dns(&self, sender: &ComponentSender<Self>) {
        let Some(conn) = self.selected_connection() else {
            return;
        };
        let uuid = conn.uuid.clone();
        let device = conn.device.clone();
        let sender = sender.clone();
        std::thread::spawn(move || {
            let label = (|| -> anyhow::Result<String> {
                if let Some(dev) = device {
                    let (v4, v6) = get_device_dns(&dev)?;
                    if !v4.is_empty() || !v6.is_empty() {
                        return Ok(format_dns_summary(&v4, &v6));
                    }
                }
                let snap = get_connection_dns(&uuid)?;
                Ok(format_dns_summary(&snap.ipv4_dns, &snap.ipv6_dns))
            })()
            .unwrap_or_else(|e| e.to_string());
            sender.input(Msg::CurrentDnsUpdated(label));
        });
    }

    fn rebuild_connection_dropdown(&self, _sender: &ComponentSender<Self>) {
        // DropDown model is set via widgets — we update through gtk root lookup is awkward
        // in Relm4 without widget refs in model. Use open display to find dropdown by rebuilding
        // via stored approach: we attach names and update in a deferred idle.
        glib::idle_add_local_once({
            let names: Vec<String> = self
                .connections
                .iter()
                .map(|c| {
                    let mark = if c.active { "● " } else { "" };
                    format!("{mark}{} ({})", c.name, c.conn_type)
                })
                .collect();
            let selected = self.selected_conn_idx as u32;
            move || {
                if let Some(win) = root_window_opt() {
                    if let Some(dd) = find_dropdown(&win, 0) {
                        let model = gtk::StringList::new(
                            &names.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                        );
                        dd.set_model(Some(&model));
                        if selected < names.len() as u32 {
                            dd.set_selected(selected);
                        }
                    }
                }
            }
        });
    }

    fn rebuild_group_dropdown(&self, _sender: &ComponentSender<Self>) {
        glib::idle_add_local_once({
            let names: Vec<String> = self.config.groups.iter().map(|g| g.name.clone()).collect();
            let selected = self.selected_group_idx as u32;
            move || {
                if let Some(win) = root_window_opt() {
                    if let Some(dd) = find_dropdown(&win, 1) {
                        let model = gtk::StringList::new(
                            &names.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                        );
                        dd.set_model(Some(&model));
                        if selected < names.len() as u32 {
                            dd.set_selected(selected);
                        }
                    }
                }
            }
        });
    }

    fn rebuild_server_list(&self, sender: &ComponentSender<Self>) {
        let group_id = self
            .config
            .groups
            .get(self.selected_group_idx)
            .map(|g| g.id.clone())
            .unwrap_or_else(|| "public".into());

        let servers: Vec<(String, String, String, bool)> = self
            .config
            .servers
            .iter()
            .filter(|s| s.group == group_id)
            .map(|s| {
                let ips = s.ipv4.join(", ");
                let lat = self
                    .latencies
                    .get(&s.id)
                    .cloned()
                    .unwrap_or_default();
                let subtitle = if lat.is_empty() {
                    ips
                } else {
                    format!("{ips} · {lat}")
                };
                let selected = self.selected_server_id.as_deref() == Some(s.id.as_str());
                (s.id.clone(), s.name.clone(), subtitle, selected)
            })
            .collect();

        let sender = sender.clone();
        glib::idle_add_local_once(move || {
            if let Some(win) = root_window_opt() {
                if let Some(list) = find_list_box(&win) {
                    while let Some(child) = list.first_child() {
                        list.remove(&child);
                    }
                    for (id, name, subtitle, selected) in servers {
                        let row = adw::ActionRow::new();
                        row.set_title(&name);
                        row.set_subtitle(&subtitle);
                        row.set_activatable(true);
                        if selected {
                            let check = gtk::Image::from_icon_name("object-select-symbolic");
                            row.add_suffix(&check);
                        }
                        let sender2 = sender.clone();
                        row.connect_activated(move |_| {
                            sender2.input(Msg::SelectServer(id.clone()));
                        });
                        list.append(&row);
                    }
                }
            }
        });
    }
}

fn root_window() -> adw::Window {
    root_window_opt().expect("application window")
}

fn root_window_opt() -> Option<adw::Window> {
    gtk::Window::list_toplevels()
        .into_iter()
        .find_map(|w| w.downcast::<adw::Window>().ok())
}

fn find_dropdown(win: &adw::Window, index: usize) -> Option<gtk::DropDown> {
    let mut found = Vec::new();
    walk_widgets(win.upcast_ref(), &mut |w| {
        if let Ok(dd) = w.clone().downcast::<gtk::DropDown>() {
            found.push(dd);
        }
    });
    found.into_iter().nth(index)
}

fn find_list_box(win: &adw::Window) -> Option<gtk::ListBox> {
    let mut found = None;
    walk_widgets(win.upcast_ref(), &mut |w| {
        if let Ok(lb) = w.clone().downcast::<gtk::ListBox>() {
            // Prefer the boxed-list used for servers
            if lb.has_css_class("boxed-list") {
                found = Some(lb);
            }
        }
    });
    found
}

fn walk_widgets(widget: &gtk::Widget, f: &mut dyn FnMut(&gtk::Widget)) {
    f(widget);
    let mut child = widget.first_child();
    while let Some(c) = child {
        walk_widgets(&c, f);
        child = c.next_sibling();
    }
}
