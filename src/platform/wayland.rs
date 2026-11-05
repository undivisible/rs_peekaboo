use std::collections::BTreeMap;

use serde_json::{Value, json};
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum,
    protocol::{wl_output, wl_registry},
};
use wayland_protocols::xdg::xdg_output::zv1::client::{zxdg_output_manager_v1, zxdg_output_v1};

use crate::{PeekabooError, Result};

#[derive(Default)]
struct Output {
    name: Option<String>,
    description: Option<String>,
    position: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
    mode: Option<(i32, i32, i32)>,
    scale: i32,
}

#[derive(Default)]
struct State {
    outputs: BTreeMap<u32, (wl_output::WlOutput, Output)>,
    manager: Option<zxdg_output_manager_v1::ZxdgOutputManagerV1>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == "wl_output" => {
                let output = registry.bind(name, version.min(4), qh, name);
                state.outputs.insert(
                    name,
                    (
                        output,
                        Output {
                            scale: 1,
                            ..Default::default()
                        },
                    ),
                );
            }
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == "zxdg_output_manager_v1" => {
                state.manager = Some(registry.bind(name, version.min(3), qh, ()));
            }
            wl_registry::Event::GlobalRemove { name } => {
                state.outputs.remove(&name);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for State {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some((_, output)) = state.outputs.get_mut(id) else {
            return;
        };
        match event {
            wl_output::Event::Name { name } => output.name = Some(name),
            wl_output::Event::Description { description } => output.description = Some(description),
            wl_output::Event::Scale { factor } => output.scale = factor,
            wl_output::Event::Mode {
                flags: WEnum::Value(flags),
                width,
                height,
                refresh,
            } if flags.contains(wl_output::Mode::Current) => {
                output.mode = Some((width, height, refresh))
            }
            _ => {}
        }
    }
}

impl Dispatch<zxdg_output_manager_v1::ZxdgOutputManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &zxdg_output_manager_v1::ZxdgOutputManagerV1,
        _: zxdg_output_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zxdg_output_v1::ZxdgOutputV1, u32> for State {
    fn event(
        state: &mut Self,
        _: &zxdg_output_v1::ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some((_, output)) = state.outputs.get_mut(id) else {
            return;
        };
        match event {
            zxdg_output_v1::Event::LogicalPosition { x, y } => output.position = Some((x, y)),
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                output.size = Some((width, height))
            }
            zxdg_output_v1::Event::Name { name } => output.name = Some(name),
            zxdg_output_v1::Event::Description { description } => {
                output.description = Some(description)
            }
            _ => {}
        }
    }
}

pub fn list_screens() -> Result<Value> {
    let error = |err| PeekabooError::System(format!("Wayland display enumeration failed: {err}"));
    let connection = Connection::connect_to_env().map_err(error)?;
    let mut queue = connection.new_event_queue();
    let handle = queue.handle();
    let _registry = connection.display().get_registry(&handle, ());
    let mut state = State::default();
    queue.roundtrip(&mut state).map_err(|err| {
        PeekabooError::System(format!("Wayland display enumeration failed: {err}"))
    })?;
    let mut xdg_outputs = Vec::new();
    if let Some(manager) = &state.manager {
        for (&id, (output, _)) in &state.outputs {
            xdg_outputs.push(manager.get_xdg_output(output, &handle, id));
        }
    }
    queue.roundtrip(&mut state).map_err(|err| {
        PeekabooError::System(format!("Wayland display enumeration failed: {err}"))
    })?;
    let screens: Vec<_> = state.outputs.into_values().map(|(_, output)| json!({
        "name": output.name,
        "description": output.description,
        "x": output.position.map(|(x, _)| x),
        "y": output.position.map(|(_, y)| y),
        "width": output.size.map(|(width, _)| width),
        "height": output.size.map(|(_, height)| height),
        "buffer_scale": output.scale,
        "mode": output.mode.map(|(width, height, refresh)| json!({"width": width, "height": height, "refresh_millihertz": refresh})),
    })).collect();
    Ok(json!({ "screens": screens }))
}
