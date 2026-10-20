use super::*;

impl Test262Runner {
    /// Prepare full JS source for execution with injected harness.
    pub fn build_source(&self, content: &str, meta: &TestMetadata) -> String {
        if meta.is_raw {
            return content.to_string();
        }

        let mut out = String::with_capacity(content.len() + 8192);

        if meta.is_only_strict {
            out.push_str("\"use strict\";\n");
        }

        // Prepend sta.js and assert.js
        if let Some(sta) = self.harness_cache.get("sta.js") {
            out.push_str(sta);
            out.push('\n');
        }
        if let Some(assert_js) = self.harness_cache.get("assert.js") {
            out.push_str(assert_js);
            out.push('\n');
        }

        // Host object shim for $262
        out.push_str("var $262 = typeof $262 !== 'undefined' ? $262 : {};\n");
        out.push_str("if (!$262.global) $262.global = typeof globalThis !== 'undefined' ? globalThis : this;\n");
        out.push_str("if (!$262.createRealm || !$262.createRealm().global) {\n");
        out.push_str("    $262.createRealm = function() {\n");
        out.push_str("        var g = typeof globalThis !== 'undefined' ? globalThis : this;\n");
        out.push_str("        return {\n");
        out.push_str("            global: g,\n");
        out.push_str("            evalScript: function(src) { return eval(src); }\n");
        out.push_str("        };\n");
        out.push_str("    };\n");
        out.push_str("}\n");
        out.push_str(
            "if (!$262.evalScript) $262.evalScript = function(src) { return eval(src); };\n",
        );
        out.push_str("if (!$262.gc) $262.gc = function() {};\n");
        out.push_str("if (!$262.agent) $262.agent = {};\n");
        out.push_str("var $DONE = function(err) { if (err) { throw err; } };\n");

        // Additional includes
        for inc in &meta.includes {
            if let Some(inc_content) = self.harness_cache.get(inc) {
                out.push_str(inc_content);
                out.push('\n');
            }
        }

        out.push_str(content);
        out
    }
}
