# header 1

## header 2

para 1 line 1
para 1 line 2
# not a header

para 2 plain *bold* _italic_ $(x^2 + 1) \sum_{k=1}^{n}1/k$  `lambda y: y + x` [linktext](url)

para 3 escaped \*bold\* escaped \_italic\_ escaped \$ sign \`lambda y: y+x\`

here's something with a footnote[^1]

[standalonelinktext](url2)

\[
\mathbb{C} \cong \frac{\mathbb{R}[t]}{(x^2 + 1)}
\]

```python
def foo(x):
    return x

def bar(y):
    return lambda n: y + n
```

![alt](foo/image.png){30}

<!--
this is a comment you shouldnt see me
-->

raw html:
<table>
    <tr>
        <th>Name</th>
        <th>Age</th>
        <th>City</th>
    </tr>
    <tr>
        <td>Alice</td>
        <td>25</td>
        <td>New York</td>
    </tr>
    <tr>
        <td>Bob</td>
        <td>30</td>
        <td>London</td>
    </tr>
</table>

>> this is a block quote

> this is not a block quote believe it or not 

this is text with a footnote[^2] to something.

## notes & errata:

[^1]: this is the first footnote defn

[^2]: this is the second footnote defn


