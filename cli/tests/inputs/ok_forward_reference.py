from dataclasses import dataclass


@dataclass
class Node:
    value: int
    next: Node | None


def total(node: Node | None) -> int:
    if node is None:
        return 0
    return node.value + total(node.next)


print(total(Node(1, Node(2, None))))
