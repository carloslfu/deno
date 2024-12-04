pub mod args;
pub mod auth_tokens;
pub mod cache;
pub mod cdp;
pub mod emit;
pub mod errors;
pub mod factory;
pub mod file_fetcher;
pub mod graph_container;
pub mod graph_util;
pub mod http_util;
pub mod js;
pub mod jsr;
pub mod lsp;
pub mod module_loader;
pub mod node;
pub mod npm;
pub mod ops;
pub mod resolver;
pub mod shared;
pub mod standalone;
pub mod task_runner;
pub mod tools;
pub mod tsc;
pub mod util;
pub mod version;
pub mod worker;

pub use crate::args::flags_from_vec;
pub use crate::args::DenoSubcommand;
pub use crate::args::Flags;
pub use crate::util::display;
pub use crate::util::v8::get_v8_flags_from_env;
pub use crate::util::v8::init_v8_flags;

use deno_core::Extension;
use deno_runtime::WorkerExecutionMode;
pub use deno_runtime::UNSTABLE_GRANULAR_FLAGS;

use deno_core::error::AnyError;
use deno_core::error::JsError;
pub use deno_npm::resolution::SnapshotFromLockfileError;
pub use deno_runtime::fmt_errors::format_js_error;
use deno_terminal::colors;
use factory::CliFactory;
use std::sync::Arc;
use tools::run::check_permission_before_script;
use tools::run::maybe_npm_install;

pub use deno_core;
pub use deno_core::op2;
pub use deno_npm;
pub use deno_runtime;
pub use deno_runtime::deno_node;

#[deno_runtime::deno_core::op2]
#[string]
fn op_my_fn() -> Option<String> {
  Some("hello".to_string())
}

deno_runtime::deno_core::extension!(
    my_extension,
    ops = [op_my_fn],
    esm_entry_point = "ext:my_extension/mod.js",
    esm = [dir "cli", "my_extension.js"],
);

pub async fn run_file(
  file_path: &str,
  mut extensions: Vec<deno_runtime::deno_core::Extension>,
) -> Result<i32, AnyError> {
  let args: Vec<_> = vec!["deno", "run", file_path]
    .into_iter()
    .map(std::ffi::OsString::from)
    .collect();

  let flags = resolve_flags_and_init(args)?;

  check_permission_before_script(&flags);

  // TODO(bartlomieju): actually I think it will also fail if there's an import
  // map specified and bare specifier is used on the command line
  let factory = CliFactory::from_flags(Arc::new(flags));
  let cli_options = factory.cli_options()?;

  let main_module = cli_options.resolve_main_module()?;

  if main_module.scheme() == "npm" {
    set_npm_user_agent();
  }

  maybe_npm_install(&factory).await?;

  let worker_factory = factory.create_cli_main_worker_factory().await?;

  let mut _extensions = std::mem::take(&mut extensions);

  _extensions.push(my_extension::init_ops_and_esm());

  let mut worker = worker_factory
    .create_main_worker(
      WorkerExecutionMode::Run,
      main_module.clone(),
      _extensions,
    )
    .await?;

  println!("👀 worker");

  let exit_code = worker.run().await?;

  println!("👀 exit_code: {:?}", exit_code);
  Ok(exit_code)
}

pub fn resolve_flags_and_init(
  args: Vec<std::ffi::OsString>,
) -> Result<Flags, AnyError> {
  let flags = match flags_from_vec(args) {
    Ok(flags) => flags,
    Err(err @ clap::Error { .. })
      if err.kind() == clap::error::ErrorKind::DisplayVersion =>
    {
      // Ignore results to avoid BrokenPipe errors.
      util::logger::init(None);
      let _ = err.print();
      std::process::exit(0);
    }
    Err(err) => {
      util::logger::init(None);
      exit_for_error(AnyError::from(err))
    }
  };

  util::logger::init(flags.log_level);

  // TODO(bartlomieju): remove in Deno v2.5 and hard error then.
  if flags.unstable_config.legacy_flag_enabled {
    println!(
            "⚠️  {}",
            (
                "The `--unstable` flag has been removed in Deno 2.0. Use granular `--unstable-*` flags instead.\nLearn more at: https://docs.deno.com/runtime/manual/tools/unstable_flags"
            )
    );
  }

  let default_v8_flags = match flags.subcommand {
    // Using same default as VSCode:
    // https://github.com/microsoft/vscode/blob/48d4ba271686e8072fc6674137415bc80d936bc7/extensions/typescript-language-features/src/configuration/configuration.ts#L213-L214
    DenoSubcommand::Lsp => vec!["--max-old-space-size=3072".to_string()],
    _ => {
      // TODO(bartlomieju): I think this can be removed as it's handled by `deno_core`
      // and its settings.
      // deno_ast removes TypeScript `assert` keywords, so this flag only affects JavaScript
      // TODO(petamoriken): Need to check TypeScript `assert` keywords in deno_ast
      vec!["--no-harmony-import-assertions".to_string()]
    }
  };

  init_v8_flags(&default_v8_flags, &flags.v8_flags, get_v8_flags_from_env());
  // TODO(bartlomieju): remove last argument once Deploy no longer needs it
  deno_core::JsRuntime::init_platform(
    None, /* import assertions enabled */ false,
  );

  Ok(flags)
}

pub fn exit_for_error(error: AnyError) -> ! {
  let mut error_string = format!("{error:?}");
  let mut error_code = 1;

  if let Some(e) = error.downcast_ref::<JsError>() {
    error_string = format_js_error(e);
  } else if let Some(SnapshotFromLockfileError::IntegrityCheckFailed(e)) =
    error.downcast_ref::<SnapshotFromLockfileError>()
  {
    error_string = e.to_string();
    error_code = 10;
  }

  exit_with_message(&error_string, error_code);
}

pub fn exit_with_message(message: &str, code: i32) -> ! {
  println!("{}: {}", "error", message.trim_start_matches("error: "));
  std::process::exit(code);
}

pub fn set_npm_user_agent() {
  static ONCE: std::sync::Once = std::sync::Once::new();
  ONCE.call_once(|| {
    std::env::set_var(
      crate::npm::NPM_CONFIG_USER_AGENT_ENV_VAR,
      crate::npm::get_npm_config_user_agent(),
    );
  });
}

pub(crate) fn unstable_exit_cb(feature: &str, api_name: &str) {
  log::error!(
    "Unstable API '{api_name}'. The `--unstable-{}` flag must be provided.",
    feature
  );
  deno_runtime::exit(70);
}
