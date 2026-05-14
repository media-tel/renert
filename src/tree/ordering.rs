use std::cmp::Ordering;
use std::sync::Arc;

use super::types::{Node, ParseChild, Tree};

/// Лексикографическое сравнение двух узлов дерева.
///
/// Используется как "второй уровень" сравнения в `Tree::cmp`, когда
/// у деревьев совпадает `range`.
///
/// Порядок критериев:
/// 1. Быстрый путь: одинаковые `Arc` => `Equal`.
/// 2. `rule_id`.
/// 3. `rank`.
/// 4. Попарное сравнение детей в порядке продукции:
///    - `Leaf` vs `Leaf` считаются равными в этой позиции;
///    - `Node` vs `Node` сравниваются рекурсивно;
///    - для смешанных типов (`Leaf`/`Node`) задан стабильный fallback-порядок.
/// 5. Если общая часть детей равна, сравнивается длина `children`.
fn cmp_nodes(a: &Arc<Node>, b: &Arc<Node>) -> Ordering {
    // Один и тот же объект в памяти.
    if Arc::ptr_eq(a, b) {
        return Ordering::Equal;
    }

    // Базовый порядок по правилу.
    if a.rule_id != b.rule_id {
        return a.rule_id.cmp(&b.rule_id);
    }

    // Затем по рангу.
    if a.rank != b.rank {
        return a.rank.cmp(&b.rank);
    }

    // Далее — по дочерним элементам в исходном порядке.
    for (left, right) in a.children.iter().zip(b.children.iter()) {
        match (left, right) {
            (ParseChild::Leaf(_), ParseChild::Leaf(_)) => continue,
            (ParseChild::Node(ln), ParseChild::Node(rn)) => {
                // Быстрый путь для идентичных подузлов.
                if Arc::ptr_eq(ln, rn) {
                    continue;
                }
                // Для дочерних узлов сначала учитывается rank, а затем rule_id/subtree:
                // это локальный приоритет для альтернатив внутри одной позиции продукции.
                if ln.rank != rn.rank {
                    return ln.rank.cmp(&rn.rank);
                }
                // Рекурсивное сравнение поддеревьев.
                let ord = cmp_nodes(ln, rn);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            // Стабильный fallback-порядок для смешанных типов детей.
            (ParseChild::Leaf(_), ParseChild::Node(_)) => return Ordering::Less,
            (ParseChild::Node(_), ParseChild::Leaf(_)) => return Ordering::Greater,
        }
    }

    // Если все общие позиции равны, короче тот, у кого меньше детей.
    a.children.len().cmp(&b.children.len())
}

/// Равенство деревьев определяется через их полный порядок (`Ord`).
impl PartialEq for Tree {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Tree {}

/// Частичный порядок делегирует полному порядку `Ord`.
impl PartialOrd for Tree {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Полный порядок деревьев.
///
/// Критерии:
/// 1. Сначала сравнивается `range`: меньший `start` идет раньше.
/// 2. При равном `start` более длинный диапазон (`больший stop`) идет раньше.
/// 3. Если `range` полностью совпадает — используется `cmp_nodes` для корней.
impl Ord for Tree {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.range == other.range {
            return cmp_nodes(&self.root, &other.root);
        }

        let (start, stop) = self.range;
        let (other_start, other_stop) = other.range;

        if start == other_start {
            return other_stop.cmp(&stop);
        }

        start.cmp(&other_start)
    }
}
