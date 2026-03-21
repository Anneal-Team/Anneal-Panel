use anneal_core::ApplicationResult;
use opentelemetry::{KeyValue, global, trace::TracerProvider as _};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{Resource, trace::SdkTracerProvider};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::settings::Settings;

pub fn init_telemetry(service_name: &str, settings: &Settings) -> ApplicationResult<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer().json();
    if let Some(endpoint) = &settings.otlp_endpoint {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter)
            .with_resource(
                Resource::builder_empty()
                    .with_attributes([KeyValue::new("service.name", service_name.to_string())])
                    .build(),
            )
            .build();
        let tracer = provider.tracer(service_name.to_string());
        global::set_tracer_provider(provider);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    }
    Ok(())
}
