use crate::app::{self, column_view::ColumnView, row_view::RowView};
use crate::rest_api;
use data_table::DataColumn;
use diwata_intel::{DataRow, TableName, Field, Tab};
use sauron::{
    html::{attributes::*, events::*, *},
    Component, Node,
};

use crate::app::{column_view, row_view};

#[derive(Debug, Clone)]
pub enum Msg {
    ColumnMsg(usize, column_view::Msg),
    RowMsg(usize, row_view::Msg),
    Scrolled((i32, i32)),
}

pub struct TableView {
    pub table_name: TableName,
    pub data_columns: Vec<DataColumn>,
    pub column_views: Vec<ColumnView>,
    pub row_views: Vec<RowView>,
    /// Which columns of the rows are to be frozen on the left side of the table
    frozen_rows: Vec<usize>,
    frozen_columns: Vec<usize>,
    pub scroll_top: i32,
    scroll_left: i32,
    allocated_width: i32,
    allocated_height: i32,
    /// the total number of rows count in the table
    total_rows: usize,
    current_page: usize,
}

impl TableView {
    pub fn from_tab(tab: Tab) -> Self {
        let data_columns = Self::fields_to_data_columns(&tab.fields);
        TableView {
            table_name: tab.table_name.clone(),
            column_views: tab
                .fields
                .iter()
                .map(|field| ColumnView::new(Self::field_to_data_column(field)))
                .collect(),
            data_columns,
            row_views: vec![],
            frozen_rows: vec![],
            frozen_columns: vec![],
            scroll_top: 0,
            scroll_left: 0,
            allocated_width: 0,
            allocated_height: 0,
            total_rows: 0,
            current_page: 0,
        }
    }

    fn fields_to_data_columns(fields: &[Field]) -> Vec<DataColumn> {
        fields.iter().map(Self::field_to_data_column).collect()
    }

    fn field_to_data_column(field: &Field) -> DataColumn {
        DataColumn {
            name: field.name.clone(),
            description: field.description.clone(),
            tags: vec![],
            data_type: field.get_data_type().clone(),
            is_primary: field.is_primary,
        }
    }

    /// replace all the data with a new data row
    /// TODO: also update the freeze_columns for each row_views
    pub fn set_data_rows(&mut self, data_row: &Vec<DataRow>, current_page: usize, total_rows: usize) {
        self.row_views = data_row
            .into_iter()
            .enumerate()
            .map(|(index, row)| RowView::new(index, row, &self.data_columns))
            .collect();
        self.update_freeze_columns();
        self.total_rows = total_rows;
        self.current_page = current_page;
    }

    pub fn freeze_rows(&mut self, rows: &Vec<usize>) {
        self.frozen_rows = rows.clone();
        self.update_frozen_rows();
    }

    /// call this is frozen rows selection are changed
    fn update_frozen_rows(&mut self) {
        let frozen_rows = &self.frozen_rows;
        self.row_views
            .iter_mut()
            .enumerate()
            .for_each(|(index, row_view)| {
                if frozen_rows.contains(&index) {
                    row_view.set_is_frozen(true)
                } else {
                    row_view.set_is_frozen(false)
                }
            })
    }

    fn frozen_row_height(&self) -> i32 {
        self.frozen_rows.len() as i32 * RowView::row_height() //use the actual row height
    }

    fn frozen_column_width(&self) -> i32 {
        self.frozen_columns.len() as i32 * 200 //use the actual column sizes for each frozen columns
    }
    /// Keep updating which columns are frozen
    /// call these when new rows are set or added
    pub fn update_freeze_columns(&mut self) {
        let frozen_columns = self.frozen_columns.clone();
        self.row_views
            .iter_mut()
            .for_each(|row_view| row_view.freeze_columns(frozen_columns.clone()))
    }

    pub fn freeze_columns(&mut self, columns: &Vec<usize>) {
        self.frozen_columns = columns.clone();
        self.update_freeze_columns();
    }

    /// This is the allocated height set by the parent tab
    pub fn set_allocated_size(&mut self, (width, height): (i32, i32)) {
        self.allocated_width = width;
        self.allocated_height = height;
    }

    /// TODO: include the height of the frozen rows
    pub fn calculate_normal_rows_size(&self) -> (i32, i32) {
        let height = self.allocated_height
            - self.frozen_row_height()
            - self.calculate_needed_height_for_auxilliary_spaces();
        let width = self.allocated_width
            - self.frozen_column_width()
            - self.calculate_needed_width_for_auxilliary_spaces();
        let clamped_height = if height < 0 { 0 } else { height };
        let clamped_width = if width < 0 { 0 } else { width };
        (clamped_width, clamped_height)
    }

    fn calculate_normal_rows_height(&self) -> i32 {
        self.calculate_normal_rows_size().1
    }

    fn calculate_normal_rows_width(&self) -> i32 {
        self.calculate_normal_rows_size().0
    }

    /// height from the columns names, padding, margins and borders
    pub fn calculate_needed_height_for_auxilliary_spaces(&self) -> i32 {
        120
    }

    pub fn calculate_needed_width_for_auxilliary_spaces(&self) -> i32 {
        80
    }

    /// calculate the height of the content
    /// it rows * row_height
    fn calculate_content_height(&self) -> i32 {
        sauron::log!("row views: {}", self.row_views.len());
        self.row_views.len() as i32 * RowView::row_height()
    }

    /// calculate the distance of the scrollbar
    /// before hitting bottom
    fn scrollbar_to_bottom(&self) -> i32 {
        let content_height = self.calculate_content_height(); // scroll height
        let row_container_height = self.calculate_normal_rows_height(); // client height
        content_height - (self.scroll_top + row_container_height)
    }

    fn is_scrolled_near_bottom(&self) -> bool {
        let scroll_bottom_allowance = 100;
        self.scrollbar_to_bottom() <= scroll_bottom_allowance
    }

    fn is_scrolled_bottom(&self) -> bool {
        self.scrollbar_to_bottom() <= 0
    }

    /// These are values in a row that is under the frozen columns
    /// Can move up and down
    fn view_frozen_columns(&self) -> Node<Msg> {
        // can move up and down
        ol(
            [
                class("frozen_columns"),
                styles([("margin-top", px(-self.scroll_top))]),
            ],
            self.row_views
                .iter()
                .enumerate()
                .filter(|(index, _row_view)| !self.frozen_rows.contains(index))
                .map(|(index, row_view)| {
                    // The checkbox selection and the rows of the frozen
                    // columns
                    div(
                        [class("selector_and_frozen_column_row")],
                        [
                            input([r#type("checkbox")], []),
                            row_view
                                .view_frozen_columns()
                                .map(move |row_msg| Msg::RowMsg(index, row_msg)),
                        ],
                    )
                })
                .collect::<Vec<Node<Msg>>>(),
        )
    }

    /// These are the columns of the frozen columns.
    /// Since frozen, they can not move in any direction
    fn view_frozen_column_names(&self) -> Node<Msg> {
        // absolutely immovable frozen column and row
        // can not move in any direction
        header(
            [class("frozen_column_names")],
            self.column_views
                .iter()
                .enumerate()
                .filter(|(index, _column)| self.frozen_columns.contains(index))
                .map(|(index, column)| {
                    column
                        .view()
                        .map(move |column_msg| Msg::ColumnMsg(index, column_msg))
                })
                .collect::<Vec<Node<Msg>>>(),
        )
    }

    /// The column names of the normal columns
    /// can move left and right and always follows the alignment of the column of the normal rows
    fn view_normal_column_names(&self) -> Node<Msg> {
        header(
            [class("normal_column_names")],
            self.column_views
                .iter()
                .enumerate()
                .filter(|(index, _column)| !self.frozen_columns.contains(index))
                .map(|(index, column)| {
                    column
                        .view()
                        .map(move |column_msg| Msg::ColumnMsg(index, column_msg))
                })
                .collect::<Vec<Node<Msg>>>(),
        )
    }

    /// The rows are both frozen row and frozen column
    /// Therefore can not move in any direction
    fn view_immovable_frozen_columns(&self) -> Node<Msg> {
        ol(
            [class("immovable_frozen_columns")],
            self.row_views
                .iter()
                .enumerate()
                .filter(|(index, _row_view)| self.frozen_rows.contains(index))
                .map(|(index, row_view)| {
                    div(
                        [class("selector_and_frozen_column_row")],
                        [
                            input([r#type("checkbox")], []),
                            row_view
                                .view_frozen_columns()
                                .map(move |row_msg| Msg::RowMsg(index, row_msg)),
                        ],
                    )
                })
                .collect::<Vec<Node<Msg>>>(),
        )
    }

    /// These are the pinned columns
    fn view_frozen_rows(&self) -> Node<Msg> {
        // can move left and right, but not up and down
        ol(
            [class("frozen_rows")],
            self.row_views
                .iter()
                .enumerate()
                .filter(|(index, _row_view)| self.frozen_rows.contains(&index))
                .map(|(index, row_view)| {
                    row_view
                        .view()
                        .map(move |row_msg| Msg::RowMsg(index, row_msg))
                })
                .collect::<Vec<Node<Msg>>>(),
        )
    }

    /// The rest of the columns and move in any direction
    fn view_normal_rows(&self) -> Node<Msg> {
        // can move: left, right, up, down
        ol(
            [
                class("normal_rows"),
                styles([
                    ("width", px(self.calculate_normal_rows_width())),
                    ("height", px(self.calculate_normal_rows_height())),
                ]),
                onscroll(Msg::Scrolled),
            ],
                self.row_views
                    .iter()
                    .enumerate()
                    .filter(|(index, _row_view)| !self.frozen_rows.contains(&index))
                    .map(|(index, row_view)| {
                        row_view
                            .view()
                            .map(move |row_msg| Msg::RowMsg(index, row_msg))
                    })
                    .collect::<Vec<Node<Msg>>>(),
            )
    }


    pub fn update(&mut self, msg: Msg) -> app::Cmd {
        match msg {
            Msg::RowMsg(row_index, row_msg) => {
                self.row_views[row_index].update(row_msg);
                app::Cmd::none()
            }
            Msg::ColumnMsg(column_index, column_msg) => {
                self.column_views[column_index].update(column_msg);
                app::Cmd::none()
            }
            Msg::Scrolled((scroll_top, scroll_left)) => {
                self.scroll_top = scroll_top;
                self.scroll_left = scroll_left;
                sauron::log!("scrollbar to bottom : {}", self.scrollbar_to_bottom());
                sauron::log!("is near scrolled bottom: {}", self.is_scrolled_near_bottom());
                sauron::log!("is scrolled bottom: {}", self.is_scrolled_bottom());
                // only in main table data
                if self.is_scrolled_near_bottom(){
                    let next_page = self.current_page + 1;
                    rest_api::fetch_window_data_next_page(&self.table_name, next_page, move|result| {
                        app::Msg::ReceivedWindowDataNextPage(next_page, result)
                     })
                }else{
                    app::Cmd::none()
                }
            }
        }
    }

    /// A grid of 2x2  containing 4 major parts of the table
    pub fn view(&self) -> Node<Msg> {
        main(
            [
                class("table"),
                // to ensure no reusing of table view when replaced with
                // another table
                key(format!("table_{}", self.table_name.complete_name())),
            ],
            [
                // TOP-LEFT: Content 1
                section(
                    [class(
                        "spacer_and_frozen_column_names_and_immovable_frozen_columns",
                    )],
                    [
                        div(
                            [class("spacer_and_frozen_column_names")],
                            [
                                div([class("spacer")], [
                                    text(format!("{}/{}", self.row_views.len(), self.total_rows)),
                                    input([r#type("checkbox"),
                                    ], [])]),
                                self.view_frozen_column_names(),
                            ],
                        ),
                        self.view_immovable_frozen_columns(),
                    ],
                ),
                // TOP-RIGHT: Content 2
                section(
                    [
                        class("normal_column_names_and_frozen_rows_container"),
                        styles([("width", px(self.calculate_normal_rows_width()))]),
                    ],
                    [section(
                        [
                            class("normal_column_names_and_frozen_rows"),
                            styles([("margin-left", px(-self.scroll_left))]),
                        ],
                        [
                            // can move left and right
                            self.view_normal_column_names(),
                            self.view_frozen_rows(),
                        ],
                    )],
                ),
                // BOTTOM-LEFT: Content 3
                // needed to overflow hide the frozen columns when scrolled up and down
                section(
                    [
                        class("frozen_columns_container"),
                        styles([("height", px(self.calculate_normal_rows_height()))]),
                    ],
                    [self.view_frozen_columns()],
                ),
                // BOTTOM-RIGHT: Content 4
                self.view_normal_rows(),
            ],
        )
    }
}
