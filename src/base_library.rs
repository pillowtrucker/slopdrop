//! The base library's procs, as they were defined at load.
//!
//! `state/stolen-treasure.tcl` is the original bot's library: about 8,500
//! procs evaluated at boot, before the tracked procs of `procs/` (a
//! tracked proc of the same name overrides it). It is not in `procs/`, so
//! `GET /api/procs` never listed it, and veles' public-reply safety layer
//! could not see what most lines call (`fedoragoonsay` → `stampsay` →
//! `STfedoragoon` all live here). [`snapshot`] records each proc the file
//! defined — name, args, body, and the same content hash a tracked proc
//! gets (`sha1("{args} {body}")`, `state.rs`), so a verdict keyed by it
//! misses the moment the proc is redefined — and `GET /api/procs?base=1`
//! serves them beside the tracked ones.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use sha1::{Digest, Sha1};
use tcl::Interpreter;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseProc {
    pub name: String,
    pub args: String,
    pub body: String,
    pub hash: String,
}

static BASE: RwLock<Vec<BaseProc>> = RwLock::new(Vec::new());

/// Every global proc and its `{args} {body}`, read from the interpreter.
pub fn definitions(interp: &Interpreter) -> HashMap<String, (String, String)> {
    let mut out = HashMap::new();
    let Ok(names) = interp.eval("info procs ::*") else {
        return out;
    };
    // An empty list must not reach `get_elements` (see `bsky::collect`).
    if names.get_string().trim().is_empty() {
        return out;
    }
    let Ok(names) = names.get_elements() else {
        return out;
    };
    for n in names {
        let name = n.get_string();
        let name = name.trim_start_matches("::").to_string();
        let args = interp
            .eval(format!("info args {{::{name}}}").as_str())
            .map(|o| o.get_string());
        let body = interp
            .eval(format!("info body {{::{name}}}").as_str())
            .map(|o| o.get_string());
        if let (Ok(a), Ok(b)) = (args, body) {
            out.insert(name, (a, b));
        }
    }
    out
}

/// The content hash a tracked proc gets.
pub fn content_hash(args: &str, body: &str) -> String {
    let mut h = Sha1::new();
    h.update(format!("{{{args}}} {{{body}}}").as_bytes());
    format!("{:x}", h.finalize())
}

/// Record the procs the base library defined: every proc that is new
/// after it loaded, or whose definition it changed (it replaces stock
/// stubs such as `magick`'s).
pub fn snapshot(interp: &Interpreter, before: &HashMap<String, (String, String)>) {
    let after = definitions(interp);
    let mut procs: Vec<BaseProc> = after
        .into_iter()
        .filter(|(name, def)| before.get(name) != Some(def))
        .map(|(name, (args, body))| BaseProc {
            hash: content_hash(&args, &body),
            name,
            args,
            body,
        })
        .collect();
    procs.sort_by(|a, b| a.name.cmp(&b.name));
    *BASE.write().unwrap_or_else(|p| p.into_inner()) = procs;
}

/// The base library's procs, minus the names `tracked` overrides.
pub fn list_except(tracked: &HashSet<String>) -> Vec<BaseProc> {
    BASE.read()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .filter(|p| !tracked.contains(&p.name))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_keeps_what_the_library_defined_or_changed() {
        let interp = Interpreter::new().unwrap();
        interp.eval("proc stub {} {error stub}; proc kept {} {return 1}").unwrap();
        let before = definitions(&interp);
        interp
            .eval("proc stub {a} {return $a}; proc fresh {x y} {list $x $y}")
            .unwrap();
        snapshot(&interp, &before);
        let all = list_except(&HashSet::new());
        let names: Vec<&str> = all.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["fresh", "stub"], "kept is stock, untouched");
        assert_eq!(all[0].args, "x y");
        assert_eq!(all[0].hash, content_hash("x y", "list $x $y"));
        let tracked: HashSet<String> = ["stub".to_string()].into();
        assert_eq!(list_except(&tracked).len(), 1, "a tracked override wins");
    }
}
