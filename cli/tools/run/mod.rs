// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::Read;
use std::sync::Arc;

use deno_config::deno_json::NodeModulesDirMode;
use deno_core::error::AnyError;
use deno_core::Extension;
use deno_runtime::WorkerExecutionMode;

use crate::args::EvalFlags;
use crate::args::Flags;
use crate::args::WatchFlagsWithPaths;
use crate::factory::CliFactory;
use crate::file_fetcher::File;

pub mod hmr;

pub fn check_permission_before_script(flags: &Flags) {
  if !flags.has_permission() && flags.has_permission_in_argv() {
    log::warn!(
      "{}",
      crate::colors::yellow(
        r#"Permission flags have likely been incorrectly set after the script argument.
To grant permissions, set them before the script argument. For example:
    deno run --allow-read=. main.js"#
      )
    );
  }
}

fn set_npm_user_agent() {
  static ONCE: std::sync::Once = std::sync::Once::new();
  ONCE.call_once(|| {
    std::env::set_var(
      crate::npm::NPM_CONFIG_USER_AGENT_ENV_VAR,
      crate::npm::get_npm_config_user_agent(),
    );
  });
}

pub async fn run_script(
  mode: WorkerExecutionMode,
  flags: Arc<Flags>,
  watch: Option<WatchFlagsWithPaths>,
) -> Result<i32, AnyError> {
  check_permission_before_script(&flags);

  if let Some(watch_flags) = watch {
    println!("watch mode disabled because extensions cannot be cloned");
    // return run_with_watch(mode, flags, watch_flags, extensions).await;
    ()
  }

  // TODO(bartlomieju): actually I think it will also fail if there's an import
  // map specified and bare specifier is used on the command line
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let deno_dir = factory.deno_dir()?;
  let http_client = factory.http_client_provider();

  // Run a background task that checks for available upgrades or output
  // if an earlier run of this background task found a new version of Deno.
  #[cfg(feature = "upgrade")]
  super::upgrade::check_for_upgrades(
    http_client.clone(),
    deno_dir.upgrade_check_file_path(),
  );

  let main_module = cli_options.resolve_main_module()?;

  if main_module.scheme() == "npm" {
    set_npm_user_agent();
  }

  maybe_npm_install(&factory).await?;

  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let mut worker = worker_factory
    .create_main_worker(mode, main_module.clone(), vec![])
    .await?;

  println!("👀 worker");

  let exit_code = worker.run().await?;

  println!("👀 exit_code: {:?}", exit_code);
  Ok(exit_code)
}

pub async fn run_from_stdin(flags: Arc<Flags>) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let main_module = cli_options.resolve_main_module()?;

  maybe_npm_install(&factory).await?;

  let file_fetcher = factory.file_fetcher()?;
  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let mut source = Vec::new();
  std::io::stdin().read_to_end(&mut source)?;
  // Save a fake file into file fetcher cache
  // to allow module access by TS compiler
  file_fetcher.insert_memory_files(File {
    specifier: main_module.clone(),
    maybe_headers: None,
    source: source.into(),
  });

  let mut worker = worker_factory
    .create_main_worker(WorkerExecutionMode::Run, main_module.clone(), vec![])
    .await?;
  let exit_code = worker.run().await?;
  Ok(exit_code)
}

pub async fn eval_command(
  flags: Arc<Flags>,
  eval_flags: EvalFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let file_fetcher = factory.file_fetcher()?;
  let main_module = cli_options.resolve_main_module()?;

  maybe_npm_install(&factory).await?;

  // Create a dummy source file.
  let source_code = if eval_flags.print {
    format!("console.log({})", eval_flags.code)
  } else {
    eval_flags.code
  };

  // Save a fake file into file fetcher cache
  // to allow module access by TS compiler.
  file_fetcher.insert_memory_files(File {
    specifier: main_module.clone(),
    maybe_headers: None,
    source: source_code.into_bytes().into(),
  });

  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let mut worker = worker_factory
    .create_main_worker(WorkerExecutionMode::Eval, main_module.clone(), vec![])
    .await?;
  let exit_code = worker.run().await?;
  Ok(exit_code)
}

pub async fn maybe_npm_install(factory: &CliFactory) -> Result<(), AnyError> {
  // ensure an "npm install" is done if the user has explicitly
  // opted into using a managed node_modules directory
  if factory.cli_options()?.node_modules_dir()?
    == Some(NodeModulesDirMode::Auto)
  {
    if let Some(npm_resolver) = factory.npm_resolver().await?.as_managed() {
      npm_resolver.ensure_top_level_package_json_install().await?;
    }
  }
  Ok(())
}
