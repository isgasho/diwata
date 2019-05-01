use crate::app::field_view::{self, FieldView};
use data_table::DataRow;
use sauron::{
    html::{attributes::*, events::*, *},
    Component, Node,
};

#[derive(Clone)]
pub enum Msg {
    FieldMsg(usize, field_view::Msg),
    DoubleClick,
}

pub struct RowView {
    fields: Vec<FieldView>,
    frozen_fields: Vec<usize>,
}

impl RowView {
    pub fn new(data_rows: DataRow) -> Self {
        RowView {
            fields: data_rows.into_iter().map(FieldView::new).collect(),
            frozen_fields: vec![],
        }
    }

    pub fn freeze_columns(&mut self, columns: Vec<usize>) {
        sauron::log!("row view freeze columns: {:?}", columns);
        self.frozen_fields = columns;
    }

    fn view_with_filter<F>(&self, filter: F) -> Node<Msg>
    where
        F: Fn(&(usize, &FieldView)) -> bool,
    {
        li(
            [class("row"), ondblclick(|_| Msg::DoubleClick)],
            self.fields
                .iter()
                .enumerate()
                .filter(filter)
                .map(|(index, field)| {
                    field
                        .view()
                        .map(move |field_msg| Msg::FieldMsg(index, field_msg))
                })
                .collect::<Vec<Node<Msg>>>(),
        )
    }

    pub fn view_frozen(&self) -> Node<Msg> {
        self.view_with_filter(|(index, _field)| self.frozen_fields.contains(index))
    }
}

impl Component<Msg> for RowView {
    fn update(&mut self, _msg: Msg) {}

    fn view(&self) -> Node<Msg> {
        self.view_with_filter(|(index, _field)| !self.frozen_fields.contains(index))
    }
}
