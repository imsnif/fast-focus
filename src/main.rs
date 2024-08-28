use kdl::KdlDocument;
use std::time::Instant;
use std::path::PathBuf;
use std::fs::{self, File};
use std::io::prelude::*;
use zellij_tile::prelude::*;

use std::collections::{HashMap, BTreeMap};

#[derive(Default)]
struct State {
    panes_to_run: BTreeMap<usize, PaneToRun>,
    own_plugin_id: Option<u32>,
    userspace_configuration: BTreeMap<String, String>,
}

register_plugin!(State);

#[derive(Debug)]
struct PaneToRun {
    pane_title: String,
    pane_id: Option<PaneId>,
}

impl PaneToRun {
    pub fn new(pane_title: String) -> Self {
        PaneToRun {
            pane_title,
            pane_id: None
        }
    }
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.userspace_configuration = configuration;
        request_permission(&[
            PermissionType::Reconfigure,
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[
            EventType::PermissionRequestResult,
            EventType::PaneUpdate,
        ]);
        self.own_plugin_id = Some(get_plugin_ids().plugin_id);
        self.parse_configuration_pane_names();
    }
    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        // TODO:
        // * create a UI for binding keys at runtime
        // * when we rerun a plugin pane, send it a convention pipe message
        let should_render = false;
        // guard against accidental broadcast messages that have similar message names
        if pipe_message.is_private {
            if let Ok(command_index) = pipe_message.name.replace('F', "").parse::<usize>() {
                self.rerun_and_focus_command_index(command_index.saturating_sub(1));
            }
        }
        should_render
    }
    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::PaneUpdate(panes) => {
                self.log_pane_ids_as_needed(panes);
            }
            Event::PermissionRequestResult(result) => {
                if result == PermissionStatus::Granted {
                    self.bind_keys();
                    hide_self();
                }
                should_render = true;
            }
            _ => (),
        };
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        // no ui for now
    }
}

impl State {
    fn bind_keys(&self) {
        let save_configuration_to_file = false;
        if let Some(plugin_id) = self.own_plugin_id {
            let mut keybinds = format!(r#"
                keybinds {{
                    shared {{
            "#);
            for key_index in self.panes_to_run.keys() {
                let key_index = key_index + 1;
                if key_index == 13 {
                    eprintln!("Can only bind 12 'F' keys");
                    break;
                }
                keybinds.push_str(&format!(r#"
                        bind "F{key_index}" {{
                            MessagePluginId {plugin_id} {{
                                name "F{key_index}"
                            }}
                        }}
                "#))
            }
            keybinds.push_str(&format!(r#"
                    }}
                }}
            "#));
            reconfigure(keybinds, save_configuration_to_file);
        }
    }
    fn rerun_and_focus_command_index(&self, command_index: usize) {
        if let Some(pane_to_run) = self.panes_to_run.get(&command_index) {
            if let Some(PaneId::Terminal(terminal_pane_id)) = pane_to_run.pane_id {
                rerun_command_pane(terminal_pane_id);
                focus_terminal_pane(terminal_pane_id, false);
            }
        }
    }
    fn parse_configuration_pane_names(&mut self) {
        if let Some(commands) = self.userspace_configuration.get("pane_names") {
            if let Ok(doc) = commands.parse::<KdlDocument>() {
                // these are in kdl format
                let mut command_index = 0;
                for node in doc.nodes() {
                    self.panes_to_run.insert(command_index, PaneToRun::new(node.name().value().trim().to_owned()));
                    command_index += 1;
                }
            }
        }
    }
    fn log_pane_ids_as_needed(&mut self, panes: PaneManifest) {
        for (_tab, panes) in panes.panes {
            for pane in panes {
                for (index, pane_to_run) in self.panes_to_run.iter_mut() {
                    if pane_to_run.pane_title == pane.title && pane_to_run.pane_id.is_none() {
                        if pane.is_plugin {
                            pane_to_run.pane_id = Some(PaneId::Plugin(pane.id));
                            rename_plugin_pane(pane.id, format!("<F{}> - {}", index + 1, pane.title));
                        } else {
                            pane_to_run.pane_id = Some(PaneId::Terminal(pane.id));
                            rename_terminal_pane(pane.id, format!("<F{}> - {}", index + 1, pane.title));
                        }
                    }
                }
            }
        }
    }
}
