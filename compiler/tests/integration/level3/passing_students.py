from dataclasses import dataclass


@dataclass
class Student:
    '''
    Represents a student and their result in a course.

    Requires grade to be between 0 and 10.
    Requires attendance to be between 0 and 100.
    '''
    name: str
    grade: float
    attendance: float


def passing_students(students: list[Student]) -> list[str]:
    '''
    Returns the names of *students* who passed, that is, obtained
    grade >= 6 and attendance >= 75.

    Examples
    >>> passing_students([])
    []
    >>> passing_students([
    ...     Student('Alfredo', 6.0, 74.0),
    ...     Student('Bianca', 5.9, 75.0),
    ...     Student('Jorge', 6.0, 75.0),
    ...     Student('Leonidas', 5.9, 74.0),
    ...     Student('Maria', 8.0, 90.0)])
    ['Jorge', 'Maria']
    '''
    passing: list[str] = []
    for student in students:
        if student.grade >= 6 and student.attendance >= 75:
            passing.append(student.name)
    return passing


assert passing_students([]) == []
assert passing_students([
Student('Alfredo', 6.0, 74.0),
Student('Bianca', 5.9, 75.0),
Student('Jorge', 6.0, 75.0),
Student('Leonidas', 5.9, 74.0),
Student('Maria', 8.0, 90.0)]) == ['Jorge', 'Maria']
