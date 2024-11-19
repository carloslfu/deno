use deno_runtime::deno_permissions::PermissionPrompter;
use deno_runtime::deno_permissions::PromptResponse;

fn main() {
  println!("Start");

  deno_runtime::deno_permissions::set_prompter(Box::new(CustomPrompter));

  deno_lib::run("./deno-lib-test/example.ts");

  println!("End");
}

struct CustomPrompter;

impl PermissionPrompter for CustomPrompter {
  fn prompt(
    &mut self,
    message: &str,
    name: &str,
    api_name: Option<&str>,
    is_unary: bool,
  ) -> PromptResponse {
    println!(
      "{}\n{} {}\n{} {}\n{} {:?}\n{} {}",
      "👀 Script is trying to access APIs and needs permission:",
      "Message:",
      message,
      "Name:",
      name,
      "API:",
      api_name,
      "Is unary:",
      is_unary
    );
    println!("Allow? [y/n]");

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
      match input.trim().to_lowercase().as_str() {
        "y" | "yes" => PromptResponse::Allow,
        _ => PromptResponse::Deny,
      }
    } else {
      println!("Failed to read input, denying permission");
      PromptResponse::Deny
    }
  }
}
