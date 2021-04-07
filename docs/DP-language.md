Armour Policy Language
======================

- [Armour Policy Language](#armour-policy-language)
  - [Read-Eval-Print-Loop (REPL)](#read-eval-print-loop-repl)
  - [Types](#types)
    - [Primitive](#primitive)
    - [Composite](#composite)
  - [Expressions, blocks and statements](#expressions-blocks-and-statements)
    - [Comments](#comments)
    - [Literals](#literals)
    - [Variables](#variables)
    - [Prefixes](#prefixes)
    - [Infixes](#infixes)
    - [Function call](#function-call)
    - [Return (early exit)](#return-early-exit)
    - [Sequences and immutable assignment (`;` and `let`)](#sequences-and-immutable-assignment--and-let)
    - [Iteration](#iteration)
    - [Conditionals](#conditionals)
      - [`if`](#if)
      - [`if .. matches ..`](#if--matches-)
      - [`if let`](#if-let)
    - [Regular expressions](#regular-expressions)
  - [Function declaration](#function-declaration)
    - [Internal](#internal)
    - [External](#external)
  - [Primitive functions](#primitive-functions)
    - [Connection::](#connection)
    - [data::](#data)
    - [Egress::](#egress)
    - [HttpResponse::](#httpresponse)
    - [HttpRequest::](#httprequest)
    - [i64::](#i64)
    - [ID::](#id)
    - [Ingress::](#ingress)
    - [IpAddr::](#ipaddr)
    - [Label::](#label)
    - [list::](#list)
    - [option::](#option)
    - [regex::](#regex)
    - [str::](#str)

<a name="repl"></a>
Read-Eval-Print-Loop (REPL)
---------------------------

You can run the policy language REPL with

```shell
$ cd armour/src
$ cargo run -p armour-lang [input file]
```

<a name="types"></a>
Types
-----

<a name="primitive-types"></a>
### Primitive

- `bool`
- `Connection`
- `data`
- `f64`
- `HttpRequest`
- `HttpResponse`
- `ID`
- `i64`
- `IpAddr`
- `Label`
- `Regex`
- `str`
- `unit` or `()`

<a name="composite-types"></a>
### Composite

- Tuples: **`( ty0, ty1, ...)`**

    ```
    > (1, false, 2.3)    
    > ((2, 2.0), (false, "a", "b"))
    ```

- Lists: **`List<ty>`**

    ```
    > [1, 2, 3]    
    > [("a", 1), ("b", 2), ("c", 3)]
    ```

- Option: **`Option<ty>`**

    ```
    > Some(1)    
    > None
    ```

<a name="expressions"></a>
Expressions, blocks and statements
----------------------------------

<a name="comments"></a>
### Comments

`... // comment ...`

<a name="literals"></a>
### Literals

| type         | example literals    |
---------------|----------------------
| `bool`       | `true`, `false`     |
| `data`       | `b"hello, world!"`  |
| `f64`        | `1.0`, `-1.1e10`    |
| `i64`        | `-123`, `123`       |
| `Label`      | `'<a>::b::*'`       |
| `regex`      | `Regex("a" | "b".*)`|
| `str`        | `"hello, world!"`   |
| `unit`, `()` | `()`                |

<a name="literals"></a>
### Variables

`[a-zA-Z][a-zA-Z0-9_]*`

<a name="prefixes"></a>
### Prefixes

| operation | symbol | type           |
----------- | -------|-----------------
| minus     | `-`    | `i64 -> i64`   |
| not       | `!`    | `bool -> bool` |

<a name="infixes"></a>
### Infixes

| operation       | symbol | type                              |
----------------- | -------|------------------------------------
| equality        | `==`   | `(<ty>, <ty>) -> bool`            |
| inequality      | `!=`   | `(<ty>, <ty>) -> bool`            |
| plus            | `+`    | `(i64, i64) -> i64`               |
| minus           | `-`    | `(i64, i64) -> i64`               |
| multiply        | `*`    | `(i64, i64) -> i64`               |
| divide          | `/`    | `(i64, i64) -> i64`               |
| remainder       | `%`    | `(i64, i64) -> i64`               |
| compare         | `<, <=, >, >=` | `(i64, i64) -> bool`      |
| and (shortcuts) | `&&`   | `(bool, bool) -> bool`            |
| or (shortcuts)  | `||`   | `(bool, bool) -> bool`            |
| concat string   | `++`   | `(str, str) -> str`               |
| concat list     | `@`    | `(List<ty>, List<ty>) -> List<ty>`|
| list membership | `in`   | `(ty, List<ty>) -> bool`          |

<a name="function-call"></a>
### Function call

```
> i64::pow(2, 8)
: 256
    
> 2.pow(8)
: 256
```

<a name="return"></a>
### Return (early exit)  

`return <expression>`

<a name="sequences"></a>
### Sequences and immutable assignment (`;` and `let`)

```
> { let x = 1; let y = 2; x + y }
: 3
    
> { let (x, y) = ((), true); y; x }
: ()
```

<a name="iteration"></a>
### Iteration

`all`, `any`, `filter`, `filter_map`, `fold`, `foreach` and `map`

```
> all [1 < 2, 2 < 4]
: true

> all x in [1, 2, 4] { x < 3 }
: false

> all x in [1, 2, 4] { x - 2 < 3 }
: true

> any [3 < 2, 2 < 4]
: true    

> any (x, y) in [(1, true), (2, false), (4, false)] { x < 3 && y }
: true

> filter x in [("x", 1"), ("y", 2), ("x", 3)] { x.0 == "x" }
: [("x", 1), ("x", 3)]

> fold x in in [1, 2, 3] { acc += x } 0
: 6 

> map x in [1, 2, 3] { x % 2 == 0 }
: [false, true, false]

> filter_map x in [1, 2, 3, 4] { if x % 2 == 0 { Some((x, 2 * x)) } else { None } }
: [(2, 4), (4, 8)]
```

### Conditionals

<a name="if"></a>
#### `if`

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

<a name="if-matches"></a>
#### `if .. matches ..`

```
if <expression1> matches <exp1> &&
   <expression2> matches <exp2> && ... {
    <unit-statement>
}

if <expression1> matches <exp1> &&
   <expression2> matches <exp2> && ... {
    <statement>
} else {
    <statement>
}
```

where each `expression<n>` and `expr<n>` are either a `str` and `Regex`, or a `Label` and a `Label`.

<a name="if-let"></a>
#### `if let`

```
if let Some(<var>) = <expr> {
    <unit-statement>
}

if let Some(<var>) = <expr> {
    <statement>
} else {
    <statement>
}
```

<a name="regular-expressions"></a>
### Regular expressions

- Literals

    `"text"`

- Classes

    ```
    :alpha:
    :alphanum:
    :base64:
    :digit:
    :hex_digit:
    :s:
    ```

- Binds (only allowed in `if match` expressions)

    ```
    [v]
    [v as i64]
    [v as base64]
    ```

- Operations

    | symbol                   | meaning           |
    -------------------------- | -------------------
    | .                        | Any               |
    | `<regexp>`?              | Optional          |
    | `<regexp>`!              | Case insensitive  |
    | `<regexp>`%              | Ignore whitespace |
    | `<regexp>`*              | Zero or more      |
    | `<regexp>`+              | One or more       |
    | `<regexp>` `<regexp>`    | Sequence          |
    | `<regexp>` \| `<regexp>` | Either            |

<a name="functions"></a>
Function declaration
--------------------

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

Primitive functions
-------------------

<a name="connection"></a>
### Connection::

function               | type
---------------------- | ----------------------------------------
| default              | `() -> Connection`                     |
| new                  | `(ID, ID, i64) -> Connection`          |
| from_to              | `Connection -> (ID, ID)`               |
| from                 | `Connection -> ID`                     |
| to                   | `Connection -> ID`                     |
| number               | `Connection -> i64`                    |
| set_from             | `(Connection, ID) -> Connection`       |
| set_to               | `(Connection, ID) -> Connection`       |
| set_number           | `(Connection, ID) -> Connection`       |

<a name="data"></a>
### data::

function               | type
---------------------- | ----------------------------------------
| len                  | `data -> i64`                          |
| to_base64            | `data -> str`                          |

<a name="egress"></a>
### Egress::

function               | type
---------------------- | ----------------------------------------
| id                   | `() -> Option<Label>`                  |
| set_id               | `() -> ()`                             |
| add_label            | `Label -> ()`                          |
| find_label            | `Label -> Label`                        |
| has_label            | `Label -> bool`                        |
| remove_label         | `Label -> ()`                          |
| data                 | `() -> List<data>`                     |
| pop                  | `() -> Option<data>`                   |
| push                 | `data -> ()`                           |
| wipe                 | `() -> ()`                             |

<a name="http-response"></a>
### HttpResponse::

function               | type
---------------------- | ----------------------------------------------
| new                  | `i64 -> HttpResponse`                        |
| connection           | `HttpResponse -> Connection`                 |
| from                 | `HttpResponse -> ID`                         |
| to                   | `HttpResponse -> ID`                         |
| status               | `HttpResponse -> i64`                        |
| version              | `HttpResponse -> str`                        |
| reason               | `HttpResponse -> Option<str>`                |
| header               | `(HttpResponse, str) -> Option<List<data>>`  |
| unique_header        | `(HttpResponse, str) -> Option<data>`        |
| headers              | `HttpResponse -> List<str>`                  |
| header_pairs         | `HttpResponse -> List<(str, data)>`          |
| set_connection       | `(HttpResponse, Connection) -> HttpResponse` |
| set_reason           | `(HttpResponse, str) -> HttpResponse `       |
| set_header           | `(HttpResponse, str, data) -> HttpResponse`  |
| set_from             | `(HttpResponse, ID) -> HttpResponse`         |
| set_to               | `(HttpResponse, ID) -> HttpResponse`         |

<a name="http-request"></a>
### HttpRequest::

function               | type
---------------------- | -------------------------------------------
| GET                  | `() -> HttpRequest`                       |
| POST                 | `() -> HttpRequest`                       |
| PUT                  | `() -> HttpRequest`                       |
| DELETE               | `() -> HttpRequest`                       |
| HEAD                 | `() -> HttpRequest`                       |
| OPTIONS              | `() -> HttpRequest`                       |
| CONNECT              | `() -> HttpRequest`                       |
| PATCH                | `() -> HttpRequest`                       |
| TRACE                | `() -> HttpRequest`                       |
| connection           | `HttpRequest -> Connection`               |
| from                 | `HttpRequest -> ID`                       |
| to                   | `HttpRequest -> ID`                       |
| method               | `HttpRequest -> str`                      |
| version              | `HttpRequest -> str`                      |
| path                 | `HttpRequest -> str`                      |
| route                | `HttpRequest -> List<str>`                |
| query                | `HttpRequest -> str`                      |
| header               | `(HttpRequest, str) -> Option<List<data>>`|
| unique_header        | `(HttpRequest, str) -> Option<data>`      |
| headers              | `HttpRequest -> List<str>`                |
| query_pairs          | `HttpRequest -> List<(str, str)>`         |
| header_pairs         | `HttpRequest -> List<(str, data)>`        |
| set_connection       | `(HttpRequest, Connection) -> HttpRequest`|
| set_path             | `(HttpRequest, str) -> HttpRequest`       |
| set_query            | `(HttpRequest, str) -> HttpRequest`       |
| set_header           | `(HttpRequest, str, data) -> HttpRequest` |
| set_from             | `(HttpRequest, ID) -> HttpRequest`        |
| set_to               | `(HttpRequest, ID) -> HttpRequest`        |

<a name="i64"></a>
### i64::

function               | type
---------------------- | ----------------------------------------
| abs                  | `i64 -> i64`                           |
| pow                  | `(i64, i64) -> i64`                    |
| min                  | `(i64, i64) -> i64`                    |
| max                  | `(i64, i64) -> i64`                    |
| to_str               | `i64 -> str`                           |

<a name="id"></a>
### ID::

function               | type
---------------------- | ----------------------------------------
| default              | `() -> ID`                             |
| labels               | `ID -> List<Label>`                    |
| hosts                | `ID -> List<str>`                      |
| ips                  | `ID -> List<IpAddr>`                   |
| port                 | `ID -> Option<i64>`                    |
| add_label            | `(ID, Label) -> ID`                    |
| add_host             | `(ID, str) -> ID`                      |
| add_ip               | `(ID, IpAddr) -> ID`                   |
| find_label           | `(ID, Label) -> Label`                  |
| has_label            | `(ID, Label) -> bool`                  |
| has_host             | `(ID, str) -> bool`                    |
| has_ip               | `(ID, IpAddr) -> bool`                 |
| set_port             | `(ID, i64) -> ID`                      |

<a name="ingress"></a>
### Ingress::

function               | type
---------------------- | ----------------------------------------
| id                   | `() -> Option<Label>`                  |
| find_label           | `Label -> Label`                        |
| has_label            | `Label -> bool`                        |
| data                 | `() -> List<data>`                     |

<a name="ipaddr"></a>
### IpAddr::

function               | type
---------------------- | ----------------------------------------
| from                 | `(i64, i64, i64, i64) -> IpAddr`       |
| octets               | `IpAddr -> (i64, i64, i64, i64)`       |
| localhost            | `() -> IpAddr`                         |
| reverse_lookup       | `IpAddr -> Option<List<str>>`          |
| lookup               | `str -> Option<List<IpAddr>>`          |

<a name="label"></a>
### Label::

function               | type
---------------------- | ----------------------------------------
| captures             | `(Label, Label) -> List<(str, str)>`   |
| parts                | `Label -> Option<List<str>>`           |

<a name="list"></a>
### list::

function               | type
---------------------- | ----------------------------------------
| len                  | `List<ty> -> i64`                      |
| reduce               | `List<ty> -> Option<ty>`               |
| is_disjoint          | `(List<ty1>, List<ty2>) -> bool`       |
| is_subset            | `(List<ty1>, List<ty2>) -> bool`       |
| difference           | `(List<ty1>, List<ty2>) -> List<ty1>`  |
| intersection         | `(List<ty1>, List<ty2>) -> List<ty1>`  |

<a name="option"></a>
### option::

function               | type
---------------------- | ----------------------------------------
| is_some              | `Option<ty> -> bool`                   |
| is_none              | `Option<ty> -> bool`                   |

<a name="regex"></a>
### regex::

function               | type
---------------------- | ----------------------------------------
| is_match             | `(Regex, str) -> bool`                 |

<a name="str"></a>
### str::

function               | type
---------------------- | ----------------------------------------
| len                  | `str -> i64`                           |
| to_lowercase         | `str -> str`                           |
| to_uppercase         | `str -> str`                           |
| trim_start           | `str -> str`                           |
| trim_end             | `str -> str`                           |
| to_base64            | `str -> str`                           |
| as_bytes             | `str -> data`                          |
| from_utf8 (lossy)    | `data -> str`                          |
| starts_with          | `(str, str) -> bool`                   |
| ends_with            | `(str, str) -> bool`                   |
| contains             | `(str, str) -> bool`                   |
| is_match             | `(str, Regex) -> bool`                 |
