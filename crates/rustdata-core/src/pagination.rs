use crate::specification::SqlValue;

#[derive(Debug, Clone)]
pub struct Page<E> {
    pub content: Vec<E>,
    pub total_elements: u64,
    pub total_pages: u64,
    pub page: u64,
    pub size: u64,
}

impl<E> Page<E> {
    pub fn new(content: Vec<E>, total_elements: u64, pageable: &Pageable) -> Self {
        let size = pageable.size.max(1);
        let total_pages = total_elements.div_ceil(size);
        Self {
            content,
            total_elements,
            total_pages,
            page: pageable.page,
            size,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn len(&self) -> usize {
        self.content.len()
    }

    pub fn is_first(&self) -> bool {
        self.page == 0
    }

    pub fn is_last(&self) -> bool {
        self.page >= self.total_pages.saturating_sub(1)
    }

    pub fn has_next(&self) -> bool {
        !self.is_last()
    }

    pub fn map<T, F: FnMut(E) -> T>(self, f: F) -> Page<T> {
        Page {
            content: self.content.into_iter().map(f).collect(),
            total_elements: self.total_elements,
            total_pages: self.total_pages,
            page: self.page,
            size: self.size,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Pageable {
    pub page: u64,
    pub size: u64,
    pub sort: Sort,
    pub filters: Vec<Filter>,
}

impl Pageable {
    pub fn of(page: u64, size: u64) -> Self {
        Self {
            page,
            size: size.max(1),
            sort: Sort::unsorted(),
            filters: Vec::new(),
        }
    }

    pub fn of_size(size: u64) -> Self {
        Self::of(0, size)
    }

    pub fn sorted(mut self, sort: Sort) -> Self {
        self.sort = sort;
        self
    }

    pub fn filtered(mut self, filters: Vec<Filter>) -> Self {
        self.filters = filters;
        self
    }

    pub fn filter(mut self, filter: Filter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn offset(&self) -> u64 {
        self.page * self.size
    }
}

impl Default for Pageable {
    fn default() -> Self {
        Self::of(0, 20)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Sort {
    pub orders: Vec<Order>,
}

impl Sort {
    pub fn by(column: impl Into<String>, direction: Direction) -> Self {
        Self {
            orders: vec![Order::new(column, direction)],
        }
    }

    pub fn ascending(column: impl Into<String>) -> Self {
        Self::by(column, Direction::Asc)
    }

    pub fn descending(column: impl Into<String>) -> Self {
        Self::by(column, Direction::Desc)
    }

    pub fn unsorted() -> Self {
        Self { orders: Vec::new() }
    }

    pub fn is_sorted(&self) -> bool {
        !self.orders.is_empty()
    }

    pub fn to_sql(&self) -> String {
        self.orders
            .iter()
            .map(|o| o.to_sql())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn and(mut self, other: Sort) -> Self {
        self.orders.extend(other.orders);
        self
    }
}

#[derive(Debug, Clone)]
pub struct Order {
    pub column: String,
    pub direction: Direction,
}

impl Order {
    pub fn new(column: impl Into<String>, direction: Direction) -> Self {
        Self {
            column: column.into(),
            direction,
        }
    }

    pub fn to_sql(&self) -> String {
        format!("{} {}", self.column, self.direction.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Asc,
    Desc,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Filter {
    pub column: String,
    pub operator: FilterOperator,
    pub value: SqlValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOperator {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Like,
    Contains,
    StartsWith,
    EndsWith,
    In,
    IsNull,
    IsNotNull,
}

impl Filter {
    pub fn to_sql(
        &self,
        dialect: crate::dialect::SqlDialect,
        param_offset: usize,
    ) -> (String, Vec<SqlValue>, usize) {
        let (sql, params) = match self.operator {
            FilterOperator::Eq => (
                format!("{} = {}", self.column, dialect.ph(param_offset)),
                vec![self.value.clone()],
            ),
            FilterOperator::Ne => (
                format!("{} != {}", self.column, dialect.ph(param_offset)),
                vec![self.value.clone()],
            ),
            FilterOperator::Lt => (
                format!("{} < {}", self.column, dialect.ph(param_offset)),
                vec![self.value.clone()],
            ),
            FilterOperator::Lte => (
                format!("{} <= {}", self.column, dialect.ph(param_offset)),
                vec![self.value.clone()],
            ),
            FilterOperator::Gt => (
                format!("{} > {}", self.column, dialect.ph(param_offset)),
                vec![self.value.clone()],
            ),
            FilterOperator::Gte => (
                format!("{} >= {}", self.column, dialect.ph(param_offset)),
                vec![self.value.clone()],
            ),
            FilterOperator::Like => (
                format!("{} LIKE {}", self.column, dialect.ph(param_offset)),
                vec![self.value.clone()],
            ),
            FilterOperator::Contains => (
                format!("{} LIKE {}", self.column, dialect.ph(param_offset)),
                vec![SqlValue::Str(format!("%{}%", self.value_to_string()))],
            ),
            FilterOperator::StartsWith => (
                format!("{} LIKE {}", self.column, dialect.ph(param_offset)),
                vec![SqlValue::Str(format!("{}%", self.value_to_string()))],
            ),
            FilterOperator::EndsWith => (
                format!("{} LIKE {}", self.column, dialect.ph(param_offset)),
                vec![SqlValue::Str(format!("%{}", self.value_to_string()))],
            ),
            FilterOperator::In => (
                format!("{} IN ({})", self.column, dialect.ph(param_offset)),
                vec![self.value.clone()],
            ),
            FilterOperator::IsNull => (format!("{} IS NULL", self.column), vec![]),
            FilterOperator::IsNotNull => (format!("{} IS NOT NULL", self.column), vec![]),
        };
        let param_count = params.len();
        (sql, params, param_offset + param_count)
    }

    fn value_to_string(&self) -> String {
        match &self.value {
            SqlValue::Str(s) => s.clone(),
            SqlValue::I64(i) => i.to_string(),
            SqlValue::I32(i) => i.to_string(),
            SqlValue::F32(f) => f.to_string(),
            SqlValue::F64(f) => f.to_string(),
            SqlValue::Bool(b) => b.to_string(),
            SqlValue::DateTime(dt) => dt.to_rfc3339(),
            _ => String::new(),
        }
    }
}
