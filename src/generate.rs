use crate::transpile::DesmoExpr;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphState {
    random_seed: String,
    expressions: Expressions,
    version: u64,
    graph: GraphMeta,
}

impl GraphState {
    pub fn from_vec(input: Vec<DesmoExpr>) -> Self {
        let mut ticker = None;
        let exprs = input
            .into_iter()
            .filter_map(|e| {
                if let Some(title) = e.content.strip_prefix("\\folder ") {
                    Some(Expression::Folder {
                        id: e.id.to_string(),
                        title: title.to_string(),
                        other: e.other,
                    })
                } else if let Some(latex) = e.content.strip_prefix("\\ticker ") {
                    ticker = Some(Ticker {
                        handler_latex: latex.to_string(),
                        open: true,
                        playing: false,
                    });
                    None
                } else {
                    Some(Expression::Expression {
                        color: None,
                        folder_id: e.folder_id.map(|x| x.to_string()),
                        id: e.id.to_string(),
                        latex: e.content,
                        other: e.other,
                    })
                }
            })
            .collect::<Vec<_>>();
        Self {
            random_seed: "desmonic".to_string(),
            expressions: Expressions {
                list: exprs,
                ticker,
            },
            version: 11,
            graph: GraphMeta {
                viewport: ViewportMeta {
                    xmin: -10.,
                    ymin: -10.,
                    xmax: 10.,
                    ymax: 10.,
                },
                show_grid: true,
                show_x_axis: true,
                show_y_axis: true,
                x_axis_numbers: true,
                y_axis_numbers: true,
                polar_numbers: false,
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Expressions {
    list: Vec<Expression>,
    ticker: Option<Ticker>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Ticker {
    handler_latex: String,
    open: bool,
    playing: bool,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Expression {
    Expression {
        color: Option<String>,
        #[serde(rename = "folderId")]
        folder_id: Option<String>,
        id: String,
        latex: String,
        #[serde(flatten)]
        other: HashMap<String, Value>,
    },
    Folder {
        id: String,
        title: String,
        #[serde(flatten)]
        other: HashMap<String, Value>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphMeta {
    viewport: ViewportMeta,
    show_grid: bool,
    show_x_axis: bool,
    show_y_axis: bool,
    x_axis_numbers: bool,
    y_axis_numbers: bool,
    polar_numbers: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewportMeta {
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
}
