use std::{any::TypeId, sync::Arc};

use weaver_util::lock::{ArcRead, ArcWrite};

use crate::prelude::{Archetype, ColumnRef, Component, Data, SparseSet, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum QueryFetchParam {
    Read(TypeId),
    Write(TypeId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum QueryFilterParam {
    With(TypeId),
    Without(TypeId),
}

#[derive(Default)]
pub struct DynFetch {
    params: Vec<QueryFetchParam>,
}

impl DynFetch {
    pub fn test_archetype(&self, archetype: &Archetype) -> bool {
        for param in &self.params {
            match param {
                QueryFetchParam::Read(type_id) => {
                    if !archetype.contains_component_by_type_id(*type_id) {
                        return false;
                    }
                }
                QueryFetchParam::Write(type_id) => {
                    if !archetype.contains_component_by_type_id(*type_id) {
                        return false;
                    }
                }
            }
        }

        true
    }
}

#[derive(Default)]
pub struct DynFilter {
    params: Vec<QueryFilterParam>,
}

impl DynFilter {
    pub fn test_archetype(&self, archetype: &Archetype) -> bool {
        for param in &self.params {
            match param {
                QueryFilterParam::With(type_id) => {
                    if !archetype.contains_component_by_type_id(*type_id) {
                        return false;
                    }
                }
                QueryFilterParam::Without(type_id) => {
                    if archetype.contains_component_by_type_id(*type_id) {
                        return false;
                    }
                }
            }
        }

        true
    }
}

pub struct QueryBuilder<'a> {
    world: &'a Arc<World>,
    fetch: DynFetch,
    filter: DynFilter,
}

impl<'a> QueryBuilder<'a> {
    pub fn new(world: &'a Arc<World>) -> Self {
        Self {
            world,
            fetch: DynFetch::default(),
            filter: DynFilter::default(),
        }
    }

    pub fn read<T: Component>(mut self) -> Self {
        self.fetch
            .params
            .push(QueryFetchParam::Read(TypeId::of::<T>()));
        self
    }

    pub fn write<T: Component>(mut self) -> Self {
        self.fetch
            .params
            .push(QueryFetchParam::Write(TypeId::of::<T>()));
        self
    }

    pub fn with<T: Component>(mut self) -> Self {
        self.filter
            .params
            .push(QueryFilterParam::With(TypeId::of::<T>()));
        self
    }

    pub fn without<T: Component>(mut self) -> Self {
        self.filter
            .params
            .push(QueryFilterParam::Without(TypeId::of::<T>()));
        self
    }

    pub fn build(self) -> DynQuery {
        let storage = self.world.storage().read();

        let mut columns = Vec::new();
        for archetype in storage.archetype_iter() {
            if self.fetch.test_archetype(archetype) && self.filter.test_archetype(archetype) {
                for param in self.fetch.params.iter() {
                    match param {
                        QueryFetchParam::Read(type_id) => {
                            let column = archetype
                                .get_column_by_type_id(*type_id)
                                .expect("Archetype should contain component");
                            columns.push(column)
                        }
                        QueryFetchParam::Write(type_id) => {
                            let column = archetype
                                .get_column_by_type_id(*type_id)
                                .expect("Archetype should contain component");
                            columns.push(column)
                        }
                    }
                }
            }
        }

        DynQuery {
            columns,
            fetch: self.fetch,
        }
    }
}

pub struct DynQuery {
    columns: Vec<ColumnRef>,
    fetch: DynFetch,
}

impl DynQuery {
    pub fn iter(&self) -> DynQueryIter {
        DynQueryIter {
            query: self,
            index: 0,
        }
    }
}

pub struct DynQueryIter<'a> {
    query: &'a DynQuery,
    index: usize,
}

impl<'a> Iterator for DynQueryIter<'a> {
    type Item = Vec<DynQueryItem>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.query.columns.len() {
            return None;
        }

        let mut items = Vec::new();
        for (column, param) in self
            .query
            .columns
            .iter()
            .zip(self.query.fetch.params.iter())
        {
            match param {
                QueryFetchParam::Read(type_id) => {
                    items.push(DynQueryItem::Ref {
                        column: column.read_arc(),
                        index: self.index,
                        type_id: *type_id,
                    });
                }
                QueryFetchParam::Write(type_id) => {
                    items.push(DynQueryItem::Mut {
                        column: column.write_arc(),
                        index: self.index,
                        type_id: *type_id,
                    });
                }
            }
        }

        self.index += 1;

        Some(items)
    }
}

pub enum DynQueryItem {
    Ref {
        column: ArcRead<SparseSet<Data>>,
        index: usize,
        type_id: TypeId,
    },
    Mut {
        column: ArcWrite<SparseSet<Data>>,
        index: usize,
        type_id: TypeId,
    },
}

impl DynQueryItem {
    pub fn as_ref<T: Component>(&self) -> Option<&T> {
        match self {
            DynQueryItem::Ref { column, index, .. } => column.dense[*index].downcast_ref(),
            DynQueryItem::Mut { column, index, .. } => column.dense[*index].downcast_ref(),
        }
    }

    pub fn as_mut<T: Component>(&mut self) -> Option<&mut T> {
        match self {
            DynQueryItem::Ref { .. } => None,
            DynQueryItem::Mut { column, index, .. } => column.dense[*index].downcast_mut(),
        }
    }

    pub fn type_id(&self) -> TypeId {
        match self {
            DynQueryItem::Ref { type_id, .. } => *type_id,
            DynQueryItem::Mut { type_id, .. } => *type_id,
        }
    }
}
