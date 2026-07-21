//! Durable contract for the shipped delegation guidance.
//!
//! The delegation reference exists because a browsing subagent once burned its
//! budget rediscovering the repository and then falsely concluded Surfari could
//! not steer an existing Browserbase context alias. These assertions fail loudly
//! if the headings, required mission-packet fields, safety phrasing, or the
//! SKILL.md -> reference pointer are ever dropped or reworded away.

use std::path::PathBuf;

fn skill_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli/ should have a repository root parent")
        .join("skill-data/core")
}

fn read(rel: &str) -> String {
    let path = skill_dir().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

fn assert_contains(haystack: &str, needle: &str, what: &str, source: &str) {
    assert!(
        haystack.contains(needle),
        "{source} lost required {what}: {needle:?}",
    );
}

#[test]
fn skill_links_the_delegation_reference() {
    let skill = read("SKILL.md");

    assert_contains(
        &skill,
        "references/delegation.md",
        "pointer to the delegation reference",
        "SKILL.md",
    );
    assert_contains(
        &skill,
        "[references/delegation.md](references/delegation.md)",
        "markdown link to the delegation reference",
        "SKILL.md",
    );
    assert_contains(
        &skill,
        "## Delegating to a subagent",
        "delegation heading",
        "SKILL.md",
    );

    // The linked file must actually exist next to the skill.
    assert!(
        skill_dir().join("references/delegation.md").is_file(),
        "SKILL.md links references/delegation.md but the file is missing",
    );
}

#[test]
fn skill_summarizes_the_delegation_contract() {
    let skill = read("SKILL.md");

    let flat = skill.replace('\n', " ");
    for phrase in [
        // Bounded delegation is encouraged, not discouraged.
        "bounded surfing subagent is encouraged",
        "mission packet",
        // Read the skill instead of searching the machine.
        "rather than searching the whole computer",
        "references/commands.md",
        // Parent keeps ownership.
        "integration owner",
        // Redaction.
        "provider session IDs, CDP URLs, credentials, or secrets",
        // Alias-only reference to browser state.
        "by alias only",
    ] {
        assert!(
            flat.contains(phrase),
            "SKILL.md lost required delegation phrase: {phrase:?}",
        );
    }
}

#[test]
fn delegation_reference_keeps_required_headings() {
    let doc = read("references/delegation.md");

    for heading in [
        "# Delegating browsing work to subagents",
        "## When to delegate",
        "## The mission packet",
        "## Read the skill, don't search the computer",
        "## The parent stays the integration owner",
        "## Never expose provider internals",
        "## Copyable mission template",
        "## Example: reusing an existing Browserbase context",
    ] {
        assert_contains(&doc, heading, "heading", "references/delegation.md");
    }
}

#[test]
fn delegation_reference_requires_every_mission_packet_field() {
    let doc = read("references/delegation.md");
    let flat = doc.replace('\n', " ");

    for field in [
        "**Objective**",
        "**Working directory and repository**",
        "**Known feature and command entrypoints**",
        "**Existing Browserbase context alias, if applicable**",
        "**Allowed actions**",
        "**Forbidden fallbacks**",
        "**Authentication and human boundary**",
        "**Expected artifacts and evidence**",
        "**Exact verification commands**",
        "**Stop condition**",
    ] {
        assert!(
            flat.contains(field),
            "references/delegation.md lost required mission-packet field: {field:?}",
        );
    }

    // The copyable template must carry the same fields, not just the prose list.
    for line in [
        "Objective:",
        "Working directory:",
        "Entrypoints (do not go looking for these):",
        "Browserbase context alias (if applicable):",
        "Allowed actions:",
        "Forbidden fallbacks:",
        "Authentication / human boundary:",
        "Expected artifacts:",
        "Verification commands",
        "Stop condition:",
    ] {
        assert_contains(
            &doc,
            line,
            "mission template line",
            "references/delegation.md",
        );
    }
}

#[test]
fn delegation_reference_keeps_safety_and_pointer_rules() {
    let doc = read("references/delegation.md");
    let flat = doc.replace('\n', " ");

    for phrase in [
        // Bounded surfing subagents are explicitly encouraged.
        "Spawning a bounded surfing subagent is encouraged",
        // Read the skill and command reference, don't sweep the filesystem.
        "read this skill and the linked command reference rather than searching the whole computer",
        "references/commands.md",
        // Parent remains the integrator.
        "The subagent surfs and reports. The parent integrates.",
        // Redaction of provider internals.
        "provider session IDs, CDP",
        "credential",
        "Refer to browser state by alias only.",
    ] {
        assert!(
            flat.contains(phrase),
            "references/delegation.md lost required phrase: {phrase:?}",
        );
    }
}

#[test]
fn delegation_example_steers_an_existing_context_by_alias() {
    let doc = read("references/delegation.md");

    for command in [
        "surfari browserbase context status eidos-docs",
        "surfari browserbase create --alias docs-sweep --context eidos-docs",
        "surfari browserbase inspect docs-sweep",
        "surfari browserbase release docs-sweep",
    ] {
        assert_contains(
            &doc,
            command,
            "existing-context example command",
            "references/delegation.md",
        );
    }

    // The example must prove alias-based steering is possible, contradicting the
    // "Surfari cannot steer an existing Browserbase alias" failure mode.
    assert!(
        doc.contains("steers it with ordinary Surfari commands, by alias"),
        "references/delegation.md lost the alias-steering claim",
    );

    // ...and it must not teach raw provider identifiers as the way in.
    for forbidden in [
        "--session-id",
        "connectUrl",
        "wss://",
        "BROWSERBASE_API_KEY",
    ] {
        assert!(
            !doc.contains(forbidden),
            "references/delegation.md must not use raw provider internals: {forbidden:?}",
        );
    }
}
