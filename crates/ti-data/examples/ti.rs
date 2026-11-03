//! Minimal CLI over `ti-data`, mainly to exercise the API.
//!
//! ```text
//! cargo run --release --example ti -- [--data DIR] [--lang en] [--scenario NAME] [--faction NAME] <command>
//!   info                      build, scenarios, factions
//!   search <query…>           full-text search
//!   show <dataName>           record summary, annotations, requirements, unlocks
//!   tree <dataName> [up|down] requirement / dependent tree
//! ```

use std::process::ExitCode;

use ti_data::graph::Traversal;
use ti_data::text::field;
use ti_data::{DataRoot, Dataset, EntityId, Scope, SearchIndex, Texts, View, model};

struct Args {
    data: String,
    lang: String,
    scenario: Option<String>,
    faction: Option<String>,
    rest: Vec<String>,
}

fn parse_args() -> Args {
    let mut a = Args { data: "data".into(), lang: "en".into(), scenario: None, faction: None, rest: vec![] };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--data" => a.data = it.next().unwrap_or_default(),
            "--lang" => a.lang = it.next().unwrap_or_default(),
            "--scenario" => a.scenario = it.next(),
            "--faction" => a.faction = it.next(),
            _ => a.rest.push(arg),
        }
    }
    a
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn label(view: &View<'_>, texts: &Texts, id: EntityId) -> String {
    let e = view.dataset().entity(id);
    format!("{} [{}:{}]", view.name(texts, id), e.template.trim_start_matches("TI").trim_end_matches("Template"), e.data_name)
}

fn resolve(view: &View<'_>, name: &str) -> Result<EntityId, String> {
    let ds = view.dataset();
    ds.find_any(name)
        .iter()
        .copied()
        .find(|id| view.contains(*id))
        .ok_or_else(|| format!("{name:?} not found in this scope"))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();
    let t = std::time::Instant::now();
    let ds: Dataset = DataRoot::open(&args.data)?.latest()?;
    let texts = Texts::load(&ds, &args.lang)?;
    let mut scope = Scope::OVERVIEW;
    if let Some(s) = &args.scenario {
        scope.scenario = Some(ds.scenario_by_name(s).ok_or_else(|| format!("unknown scenario {s:?}"))?.idx);
    }
    if let Some(f) = &args.faction {
        scope.faction = Some(ds.find(model::FACTION, f).ok_or_else(|| format!("unknown faction {f:?}"))?);
    }
    let view = ds.view(scope);
    eprintln!("loaded build {} ({}) in {:?}", ds.build_id(), ds.game_version().unwrap_or("?"), t.elapsed());

    let cmd = args.rest.first().map(String::as_str).unwrap_or("info");
    match cmd {
        "info" => {
            println!("entities: {}, languages: {}", ds.entities().len(), ds.languages().join(" "));
            for s in ds.scenarios() {
                let start = s.start_time.map(|id| ds.entity(id).data_name.to_string()).unwrap_or_default();
                println!(
                    "scenario {:<20} listed={:<5} tags={:?} start={} dlc={:?}",
                    s.data_name, s.listed, s.tags, start, s.required_dlc
                );
            }
            for f in view.entities(model::FACTION) {
                println!("faction {}", label(&view, &texts, f));
            }
            let g = view.research_graph();
            println!(
                "research graph: {} nodes, {} roots, max depth {}, cyclic {}",
                g.nodes().len(),
                g.roots().count(),
                g.nodes().iter().filter_map(|n| g.depth(*n)).max().unwrap_or(0),
                g.cyclic_nodes().len()
            );
        }
        "search" => {
            let q = args.rest[1..].join(" ");
            let t = std::time::Instant::now();
            let index = SearchIndex::build(&ds, &texts);
            let built = t.elapsed();
            let t = std::time::Instant::now();
            let hits = index.search(&q, 20, &|id| view.contains(id));
            eprintln!("index {built:?}, query {:?}", t.elapsed());
            for h in hits {
                println!("{:>5} {:?}\t{}", h.score, h.field, label(&view, &texts, h.id));
            }
        }
        "show" => {
            let id = resolve(&view, args.rest.get(1).ok_or("show <dataName>")?)?;
            println!("{}", label(&view, &texts, id));
            for f in [field::SUMMARY, field::DESCRIPTION, field::QUOTE] {
                if let Some(t) = view.text(&texts, id, f) {
                    println!("  {f}: {}", t.replace('\n', " / "));
                }
            }
            println!("  {:?}", view.annotations(id));
            println!("  icon: {:?}", view.icon_resource(id).and_then(|r| ds.asset(&r).map(|a| a.file.clone())));
            let graph = view.research_graph();
            for (i, g) in graph.requirements(id).iter().enumerate() {
                let opts: Vec<String> = g
                    .options_with_kind()
                    .map(|(o, alt)| format!("{}{}", if alt { "alt " } else { "" }, label(&view, &texts, o)))
                    .collect();
                let hidden: Vec<String> = g.hidden.iter().map(|o| label(&view, &texts, *o)).collect();
                println!("  requires[{i}]: {} hidden {hidden:?} unresolved {:?}", opts.join(" OR "), g.unresolved);
            }
            if !graph.dropped_prereqs(id).is_empty() {
                println!("  dropped prereqs: {:?}", graph.dropped_prereqs(id));
            }
            if graph.contains(id) {
                println!("  obtainable: {}", graph.is_obtainable(id));
            }
            for e in view.effects(id) {
                println!("  effect: {}", label(&view, &texts, e));
            }
            for u in view.unlocks(id) {
                println!("  unlocks {:?}: {}", u.kind, label(&view, &texts, u.entity));
            }
            for by in view.unlocked_by(id) {
                println!("  unlocked by: {}", label(&view, &texts, by));
            }
        }
        "tree" => {
            let id = resolve(&view, args.rest.get(1).ok_or("tree <dataName> [up|down]")?)?;
            let g = view.research_graph();
            let sub = match args.rest.get(2).map(String::as_str) {
                Some("down") => g.descendants(id, Traversal::default()),
                _ => g.ancestors(id, Traversal { alternatives: false, max_depth: None }),
            };
            println!(
                "{} nodes, {} edges, total research cost {}, obtainable: {}",
                sub.nodes.len(),
                sub.edges.len(),
                sub.research_cost(&view),
                g.is_obtainable(id)
            );
            let mut nodes: Vec<_> = sub.nodes.iter().copied().collect();
            nodes.sort_by_key(|n| (g.depth(*n), *n));
            for n in nodes {
                println!("  d{:<2} {}", g.depth(n).unwrap_or(0), label(&view, &texts, n));
            }
        }
        other => return Err(format!("unknown command {other:?}").into()),
    }
    Ok(())
}
