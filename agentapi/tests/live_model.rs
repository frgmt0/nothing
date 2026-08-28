use nothing_action::log::AuthorId;
use nothing_agentapi::holectx::hole_context;
use nothing_agentapi::measure::claude::{Claude, first_meaningful_line};
use nothing_agentapi::session::AgentSession;

fn claude() -> Claude {
    Claude::new()
}

#[test]
#[ignore = "calls the claude CLI; run with --ignored when a real model is wanted"]
fn the_claude_cli_answers_a_trivial_prompt() {
    let reply = claude()
        .ask("Reply with exactly the word OK and nothing else.")
        .expect("the claude CLI answers");
    assert!(
        reply.text.to_ascii_uppercase().contains("OK"),
        "got `{}`",
        reply.text
    );
}

#[test]
#[ignore = "calls the claude CLI; run with --ignored when a real model is wanted"]
fn a_model_picks_one_offered_construction_at_a_num_hole() {
    let mut session = AgentSession::new(AuthorId::new(2));
    for step in [
        "construct-lam",
        "move-parent",
        "rename n",
        "set-ann Num",
        "move-child 0",
        "construct-var n",
        "construct-binop mul",
    ] {
        assert!(session.apply_text(step).unwrap(), "{step}");
    }

    let context = hole_context(session.state());
    let prompt = format!(
        "You are editing a program in a structural editor. Make the function double \
         its argument.\n\nCurrent program:\n  {}\n\n{}\nAnswer with exactly one \
         action, on one line, and nothing else.\n",
        session.state().render(),
        context.to_prompt_block()
    );

    let reply = claude().ask(&prompt).expect("the claude CLI answers");
    let action = first_meaningful_line(&reply.text);
    assert!(
        session.apply_text(&action).unwrap_or(false),
        "the model answered `{action}`, which did not apply"
    );
    assert!(nothing_core::typing::is_well_typed(&session.exp()));
}
