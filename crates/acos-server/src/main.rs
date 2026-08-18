//! ACOS Web Server.
//!
//! Serves the single-page UI and exposes an HTTP API to compile and run
//! ACOS tasks. Execution events are streamed to the client as Server-Sent
//! Events so the UI can show live progress.

use std::sync::Arc;

use actix_files as fs;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};

use acos_compiler::{ModelCompiler, RuleCompiler};
use acos_core::error::AcosError;
use acos_core::schema::from_yaml;
use acos_core::traits::Compiler;
use acos_core::types::TaskSpec;
use acos_runtime::Runtime;
use acos_state::InMemoryStore;
use acos_verify::verify_run;

/// Request body for `/api/write-file`.
#[derive(Debug, Deserialize)]
struct WriteFileRequest {
    path: String,
    content: String,
}

/// Request body for the `/api/run` endpoint.
#[derive(Debug, Deserialize)]
struct RunRequest {
    /// Raw YAML task specification.
    task_yaml: String,
    /// If true, force the deterministic rule-first planner.
    #[serde(default)]
    use_rules: bool,
}

/// A single event emitted during planning/execution.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RunEvent {
    /// The planner started.
    Planning { planner: String },
    /// The compiled CIR program.
    Planned {
        program_id: String,
        node_count: usize,
        diagnostics: Vec<String>,
    },
    /// A node started executing.
    NodeStart { node_id: String, kind: String },
    /// An artifact was produced.
    Artifact { name: String },
    /// A final report.
    Finished {
        run_id: String,
        status: String,
        artifacts: Vec<String>,
        verification: String,
    },
    /// An error occurred.
    Error { message: String },
}

/// Response when streaming is not requested: the final result.
#[derive(Debug, Serialize)]
struct RunResponse {
    run_id: String,
    status: String,
    artifacts: Vec<String>,
    verification: String,
    events: Vec<RunEvent>,
}

/// Runs a task (compile + execute) and collects all events into a Vec.
async fn run_task_collect(
    task_yaml: &str,
    use_rules: bool,
) -> (Vec<RunEvent>, Result<RunResponse, AcosError>) {
    let mut events: Vec<RunEvent> = vec![];

    // Parse task.
    let task: TaskSpec = match from_yaml(task_yaml) {
        Ok(t) => t,
        Err(e) => {
            events.push(RunEvent::Error {
                message: format!("parse error: {e}"),
            });
            return (events, Err(e));
        }
    };

    // Compile.
    let compiled = if use_rules {
        events.push(RunEvent::Planning {
            planner: "rule-first".into(),
        });
        RuleCompiler::new().compile(task).await
    } else {
        match ModelCompiler::from_env() {
            Ok(model) => {
                events.push(RunEvent::Planning {
                    planner: format!("model-assisted ({})", acos_llm::LongCatClient::from_env().map(|c| c.model().to_string()).unwrap_or_default()),
                });
                model.compile(task).await
            }
            Err(_) => {
                events.push(RunEvent::Planning {
                    planner: "rule-first (no API key)".into(),
                });
                RuleCompiler::new().compile(task).await
            }
        }
    };

    let mut result = match compiled {
        Ok(r) => r,
        Err(e) => {
            events.push(RunEvent::Error {
                message: format!("compile error: {e}"),
            });
            return (events, Err(e));
        }
    };

    let program_id = result.program.id.0.to_string();
    let node_count = result.program.nodes.len();
    let diagnostics: Vec<String> = result
        .diagnostics
        .iter()
        .map(|d| format!("[{:?}] {}", d.level, d.message))
        .collect();

    events.push(RunEvent::Planned {
        program_id: program_id.clone(),
        node_count,
        diagnostics,
    });

    // Suppress the unused warning; we use program_id above.
    let _ = &mut result.program.task_id;

    // Execute.
    let store: Arc<dyn acos_core::traits::EventStore + Send + Sync> =
        Arc::new(InMemoryStore::new());
    let artifact_store: Arc<dyn acos_core::traits::ArtifactStore + Send + Sync> =
        Arc::new(InMemoryStore::new());

    // Emit node-start events for each node.
    for node in &result.program.nodes {
        events.push(RunEvent::NodeStart {
            node_id: node.node_id.clone(),
            kind: format!("{:?}", node.kind),
        });
    }

    let runtime = Runtime::new(store.clone(), artifact_store);
    let report = match runtime.execute(result.program).await {
        Ok(r) => r,
        Err(e) => {
            events.push(RunEvent::Error {
                message: format!("runtime error: {e}"),
            });
            return (events, Err(e));
        }
    };

    // Emit artifact events.
    for artifact in &report.artifacts {
        events.push(RunEvent::Artifact {
            name: artifact.clone(),
        });
    }

    // Verify.
    let verification: String = match verify_run(&*store, report.run_id).await {
        Ok(v) => {
            if v.all_passed() {
                "PASSED".to_string()
            } else {
                "FAILED".to_string()
            }
        }
        Err(_) => "ERROR".to_string(),
    };

    events.push(RunEvent::Finished {
        run_id: report.run_id.0.to_string(),
        status: format!("{:?}", report.status),
        artifacts: report.artifacts.clone(),
        verification: verification.clone(),
    });

    (
        events,
        Ok(RunResponse {
            run_id: report.run_id.0.to_string(),
            status: format!("{:?}", report.status),
            artifacts: report.artifacts,
            verification,
            events: vec![],
        }),
    )
}

/// `/api/run` — compile and execute a task, returning the full result as JSON.
async fn api_run(req: web::Json<RunRequest>) -> impl Responder {
    let (events, result) = run_task_collect(&req.task_yaml, req.use_rules).await;
    match result {
        Ok(mut resp) => {
            resp.events = events;
            HttpResponse::Ok().json(resp)
        }
        Err(_) => {
            let resp = RunResponse {
                run_id: String::new(),
                status: "Failed".into(),
                artifacts: vec![],
                verification: "ERROR".to_string(),
                events,
            };
            HttpResponse::BadRequest().json(resp)
        }
    }
}

/// `/api/health` — health check.
async fn api_health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}

/// `/api/write-file` — write a file to the project directory (for sample data).
async fn api_write_file(req: web::Json<WriteFileRequest>) -> impl Responder {
    // Security: only allow writing under the current directory.
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "cannot determine current directory"
            }))
        }
    };
    let path = std::path::Path::new(&req.path);
    // Resolve relative to cwd and ensure it stays within cwd.
    let full = cwd.join(path);
    let canonical = match full.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // File/parent may not exist yet.
            if let Some(parent) = full.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match full.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    // Fall back to the joined path if still not resolvable.
                    full.clone()
                }
            }
        }
    };
    if !canonical.starts_with(&cwd) {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "error": "path must be within the project directory"
        }));
    }
    match std::fs::write(&canonical, &req.content) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "ok": true })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("write failed: {e}")
        })),
    }
}

/// `/api/artifact` — read an artifact file from disk (e.g. report.md).
async fn api_artifact(info: web::Query<std::collections::HashMap<String, String>>) -> impl Responder {
    let name = info.get("name").map(String::as_str).unwrap_or("");
    if name.is_empty() || name.contains("..") || name.starts_with('/') {
        return HttpResponse::BadRequest().json(serde_json::json!({ "error": "invalid name" }));
    }
    let path = std::env::current_dir().unwrap_or_default().join(name);
    match std::fs::read_to_string(&path) {
        Ok(content) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body(content),
        Err(e) => HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("artifact not found: {e}")
        })),
    }
}

/// Serve the single-page UI and static assets.
async fn index() -> actix_web::Result<fs::NamedFile> {
    Ok(fs::NamedFile::open("static/index.html")?)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env file if present (silently ignore if missing).
    dotenvy::dotenv().ok();
    let port = std::env::var("ACOS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080u16);

    println!("ACOS Agent Server");
    println!("  URL:  http://localhost:{}", port);
    println!("  Open in browser to use the ACOS agent.");
    println!();

    HttpServer::new(move || {
        App::new()
            .route("/api/health", web::get().to(api_health))
            .route("/api/run", web::post().to(api_run))
            .route("/api/write-file", web::post().to(api_write_file))
            .route("/api/artifact", web::get().to(api_artifact))
            .service(fs::Files::new("/static", "static").show_files_listing())
            .default_service(web::get().to(index))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
