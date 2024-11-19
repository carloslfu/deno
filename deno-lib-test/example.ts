import { walk } from "https://esm.sh/jsr/@std/fs@0.218.2/walk";

for await (const entry of walk(Deno.env.get("HOME") + "/Desktop")) {
  if (entry.isFile) {
    console.log(`File: ${entry.path}`);
  } else if (entry.isDirectory) {
    console.log(`Directory: ${entry.path}`);
  }
}
