use super::{LogRecord, shared};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{Layer, layer::Context};

pub struct DiagnosticsLayer;
#[derive(Default)]
struct Fields {
    message: String,
    details: String,
    plugin: Option<String>,
    generation: Option<String>,
}
impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message = value.into(),
            "plugin_id" => self.plugin = Some(value.into()),
            "generation" => self.generation = Some(value.into()),
            _ => {
                self.details
                    .push_str(&format!("{}: {value}\n", field.name()));
            },
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_str(field, &format!("{value:?}"));
    }
}
impl<S: Subscriber> Layer<S> for DiagnosticsLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let metadata = event.metadata();
        let plugin_source = fields.plugin.is_some() || metadata.target().contains("asio_adapter");
        let mut record = LogRecord::new(
            if plugin_source { "plugin" } else { "rust" },
            metadata.level().as_str(),
            metadata.target(),
            fields.message,
            fields.details,
        );
        record.plugin_id = fields.plugin;
        record.generation = fields.generation;
        shared().record(record);
    }
}
