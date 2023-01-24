use crate::build::fetch::fetch_source;
use crate::source::{Execution, Source};
use anyhow::bail;
use rhai::{Dynamic, Engine, FnPtr, Scope, AST};
use std::fs::read_to_string;
use std::path::Path;
use std::process::Command;
use tempfile::{tempdir, TempDir};

#[derive(Debug)]
pub struct BuildScript {
  engine: Engine,
  ast: AST,
  source: Source,
  source_dir: TempDir,
}

impl BuildScript {
  pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
    let engine = Engine::new();
    let mut scope = Scope::new();
    let source_dir = tempdir()?;
    let source_dir_path = source_dir
      .path()
      .to_str()
      .expect("tempdir path is not UTF-8")
      .to_string();

    scope.push("source_dir", source_dir_path);

    let ast = engine.compile_with_scope(&mut scope, &read_to_string(path)?)?;
    let mut value = engine.eval_ast_with_scope(&mut scope, &ast)?;
    let source = Source::from_dynamic(&mut value)?;
    Ok(Self {
      engine,
      ast,
      source,
      source_dir,
    })
  }

  pub fn source(&self) -> &Source {
    &self.source
  }

  fn exec_shell(&self, dir: impl AsRef<Path>, x: &str) -> anyhow::Result<()> {
    let status = Command::new("sh")
      .args(["-c", &format!("set -e\n{x}")])
      .current_dir(dir)
      .status()?;
    if !status.success() {
      bail!("Shell exited with {status}");
    }
    Ok(())
  }

  fn exec_fn(&self, dir: impl AsRef<Path>, f: &FnPtr) -> anyhow::Result<()> {
    let result: Dynamic = f.call(&self.engine, &self.ast, ())?;
    if let Ok(x) = result.into_string() {
      self.exec_shell(dir, &x)?;
    }
    Ok(())
  }

  fn exec(&self, dir: impl AsRef<Path>, x: &Execution) -> anyhow::Result<()> {
    match x {
      Execution::Shell(x) => self.exec_shell(dir, x),
      Execution::Fn(f) => self.exec_fn(dir, f),
    }
  }

  pub fn prepare(&self) -> anyhow::Result<()> {
    let source_dir = self.source_dir.path();
    println!(":: Fetching source...");
    fetch_source(source_dir, &self.source.meta.source)?;
    // TODO: dependency check
    println!(":: Executing `prepare`...");
    if let Some(prepare) = &self.source.prepare {
      self.exec(source_dir, prepare)?;
    }
    Ok(())
  }

  pub fn build(&self) -> anyhow::Result<()> {
    if let Some(build) = &self.source.build {
      self.exec(self.source_dir.path(), build)?;
    }
    Ok(())
  }

  pub fn pack(&self) -> anyhow::Result<()> {
    todo!()
  }
}
