Armour Policy Language
======================

## Command Line

It is recommended to start the policy language REPL (Read-Eval-Print loop) with

    $ rlwrap cargo run [input file]

[`rlwrap` can be installed on a Mac with `brew install rlwrap`.]

## Types

### Primitive

- **`bool`**  

    `true`, `false`

- **`i64`**  

    `-123`, `123`
    
- **`f64`**  

    `1.0`, `-1.1e10`

- **`str`**

    `"hello, world!"`

- **`data`**

    `b"hello, world!"`

- **`unit`** or **`()`**

    `()`

- **`Policy`**

    `Accept`, `Reject`, `Forward`

- **`HttpRequest`**

### Composite

- Tuples: **`( ty0, ty1, ...)`**

    ```
    (1, false, 2.3)
    
    ((2, 2.0), (false, "a", "b"))
    ```

- Lists: **`List<ty>`**

    ```
    [1, 2, 3]
    
    [("a", 1), ("b", 2), ("c", 3)]
    ```

## Expressions, blocks and statements

- Literals (see above)
- Variables

    `[a-zA-Z][a-zA-Z0-9_]*`

- Prefixes

    | operation | symbol and type    |
    ----------- | --------------------
    | minus     | `- : i64 -> i64`   |
    | not       | `! : bool -> bool` |

- Infixes

    | operation       | symbol and type                        |
    ----------------- | ----------------------------------------
    | equality        | `== : (<ty>, <ty>) -> bool`            |
    | inequality      | `!= : (<ty>, <ty>) -> bool`            |
    | plus            | `+ : (i64, i64) -> i64`                |
    | minus           | `- : (i64, i64) -> i64`                |
    | multiply        | `* : (i64, i64) -> i64`                |
    | divide          | `/ : (i64, i64) -> i64`                |
    | remainder       | `% : (i64, i64) -> i64`                |
    | compare         | `<, <=, >, >= : (i64, i64) -> bool`    |
    | and (shortcuts) | `&& : (bool, bool) -> bool`            |
    | or (shortcuts)  | `|| : (bool, bool) -> bool`            |
    | concat string   | `++ : (str, str) -> str`               |
    | concat list     | `@ : (List<ty>, List<ty>) -> List<ty>` |
    | list membership | `in : (List<ty>, List<ty>) -> bool`    |

- Function call

    ```
    > i64::pow(2, 8)
    : 256
    
    > 2.pow(8)
    : 256
    ```

- Function exit  

    `return <expression>`

- Sequences **;** and immutable **let** assigment

    ```
    > { let x = 1; let y = 2; x + y }
    : 3
    
    > { let (x, y) = ((), true); y; x }
    : true
    ```

- **all**, **any**, **filter**, **map**

    ```
    > all x in [1, 2, 4] { x < 3 }
    : false
    
    > all x in [1, 2, 4] { x - 2 < 3 }
    : true

    > any (x, y) in [(1, true), (2, false), (4, false)] { x < 3 && y }
    : true

    > filter x in [("x", 1"), ("y", 2), ("x", 3)] { x.0 == "x" }
    : [("x", 1), ("x", 3)]
    
    > map x in [1, 2, 3] { x % 2 == 0 }
    : [false, true, false]
    ```

- **if**

    ```
    if <bool-expression> { <unit-statement> }
    
    if <bool-expression> { <statement> } else { <statement> }
    
    if <bool-expression> {
        <unit-statement>
    } else if <bool-expression> {
        <unit-statement>
    }
    
    if <bool-expression> {
        <statement>
    else if <bool-expression> {
        <statement>
    } else {
        <statement>
    }
    ```

- **if match**

    ```
    if <str-expression1> matches <pat1> and
       <str-expression1> matches <pat1> and ... {
        <unit-statement>
    }

    if <str-expression1> matches <pat1> and
       <str-expression1> matches <pat1> and ... {
        <statement>
    } else {
        <statement>
    }
    ```

## Patterns

- Literals

    `"text"`

- Classes

    ```
    :alpha:
    :alphanum:
    :digit:
    :hex_digit:
    :s:
    ```

- Binds

    ```
    [v]
    [v as i64]
    [v as base64]
    ```

- Operations

    | Symbol               | Meaning           |
    ---------------------- | -------------------
    | .                    | Any               |
    | `<pat>`?             | Optional          |
    | `<pat>`!             | Case insensitive  |
    | `<pat>`%             | Ignore whitespace |
    | `<pat>`*             | Zero or more      |
    | `<pat>`+             | One or more       |
    | `<pat1>` `<pat2>`    | Sequence          |
    | `<pat1>` \| `<pat2>` | Either            |

## Function declarations

### Internal

```
fn <name>() -> <ty> { <statement> }

fn <name>() { <unit-statement> }

fn <name>(<arg1>: <ty1>, <arg2>: <ty2>, ...) -> <ty> { <statement> }

fn <name>(<arg1>: <ty1>, <arg2>: <ty2>, ...) { <unit-statement> }
```

### External

```
external <external name> @ "<url>" {
  fn <name>()
  fn <name>() -> <ty>
  fn <name>(<ty1>, <ty2>, ...)
  fn <name>(<ty1>, <ty2>, ...) -> <ty>
  ...
}
```

## Builtin functions

### HttpRequest::

function               | type
---------------------- | ------------------------------------------
| default              | `() -> HttpRequest`                      |
| method               | `HttpRequest -> str`                     |
| version              | `HttpRequest -> str`                     |
| path                 | `HttpRequest -> str`                     |
| route                | `HttpRequest -> List<str>`               |
| query                | `HttpRequest -> str`                     |
| header               | `(HttpRequest, str) -> str`              |
| headers              | `HttpRequest -> List<str>`               |
| query_pairs          | `HttpRequest -> List<(str, str)>`        |
| header_pairs         | `HttpRequest -> List<(str, data)>`       |
| set_path             | `(HttpRequest, str) -> HttpRequest`      |
| set_query            | `(HttpRequest, str) -> HttpRequest`      |
| set_header           | `(HttpRequest, str, str) -> HttpRequest` |

### i64::

function               | type
---------------------- | ------------------------------------------
| abs                  | `i64 -> i64`                             |
| pow                  | `(i64, i64) -> i64`                      |
| min                  | `(i64, i64) -> i64`                      |
| max                  | `(i64, i64) -> i64`                      |
| to_str               | `i64 -> str`                             |

### str::

function               | type
---------------------- | ------------------------------------------
| len                  | `str -> i64`                             |
| to_lowercase         | `str -> str`                             |
| to_uppercase         | `str -> str`                             |
| trim_start           | `str -> str`                             |
| trim_end             | `str -> str`                             |
| to_base64            | `str -> str`                             |
| as_bytes             | `str -> data`                            |
| from_utf8 (lossy)    | `data -> str`                            |
| starts_with          | `(str, str) -> bool`                     |
| ends_with            | `(str, str) -> bool`                     |
| contains             | `(str, str) -> bool`                     |

### data::

function               | type
---------------------- | ------------------------------------------
| len                  | `data -> i64`                            |
| to_base64            | `data -> str`                             |

### list::

function               | type
---------------------- | ------------------------------------------
| len                  | `List<ty> -> i64`                         |
