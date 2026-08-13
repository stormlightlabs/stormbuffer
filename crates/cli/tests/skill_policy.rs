#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    Continue,
    RecallAndCite,
    ProposeCandidate,
    UpdateStale,
    CreateCheckpoint,
}

#[derive(Clone, Copy)]
enum Event {
    None,
    Recall,
    Capture,
    StaleMemory,
    NecessaryHandoff,
}

fn decide(event: Event, rejected: bool, artifacts_sufficient: bool) -> Outcome {
    match event {
        Event::Recall => Outcome::RecallAndCite,
        Event::None => Outcome::Continue,
        _ if rejected || artifacts_sufficient => Outcome::Continue,
        Event::StaleMemory => Outcome::UpdateStale,
        Event::NecessaryHandoff => Outcome::CreateCheckpoint,
        Event::Capture => Outcome::ProposeCandidate,
    }
}

#[test]
fn skill_policy_routes_memory_actions() {
    let scenarios = [
        (
            "no capture event",
            Event::None,
            false,
            false,
            Outcome::Continue,
        ),
        (
            "prior context",
            Event::Recall,
            false,
            false,
            Outcome::RecallAndCite,
        ),
        (
            "durable capture",
            Event::Capture,
            false,
            false,
            Outcome::ProposeCandidate,
        ),
        (
            "rejected capture",
            Event::Capture,
            true,
            false,
            Outcome::Continue,
        ),
        (
            "stale memory",
            Event::StaleMemory,
            false,
            false,
            Outcome::UpdateStale,
        ),
        (
            "necessary handoff",
            Event::NecessaryHandoff,
            false,
            false,
            Outcome::CreateCheckpoint,
        ),
        (
            "handoff preserved by repository artifacts",
            Event::NecessaryHandoff,
            false,
            true,
            Outcome::Continue,
        ),
    ];

    for (name, event, rejected, artifacts_sufficient, expected) in scenarios {
        assert_eq!(
            decide(event, rejected, artifacts_sufficient),
            expected,
            "{name}"
        );
    }
}
