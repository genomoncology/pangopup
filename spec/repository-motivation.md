# Why this repository exists

A reader who opens `README.md` and follows one link learns why this project
exists and what calling this software from other software means for that
software's own licence. These checks inspect committed text only; they do not
use the network, synchronize assets, start a service, or remove files.

`README.md` carries that one link, in the section that already states this
project's own licence, and the sentence holding it names both of the things the
link answers. One link, so a reader has one place to go.

```bash
citation=$(awk '/^## Citation and license$/ { on=1; next } on' ../README.md)
printf '%s' "$citation" | rg -F '](architecture/motivation.md)' >/dev/null
sentence=$(printf '%s' "$citation" | tr '\n' ' ' | tr -s ' ' \
  | sed -E 's/\. /.\n/g' | rg -F '](architecture/motivation.md)')
printf '%s' "$sentence" | rg -i '\b(exists|existed|motivation|built)\b' >/dev/null
printf '%s' "$sentence" | rg -i '\blicen[cs]e\b' >/dev/null
test "$(rg -c -F '](architecture/motivation.md)' ../README.md)" = 1
printf 'one README link reaches why this project exists\n' | mustmatch like 'one README link reaches why this project exists'
```

The material behind that link states every claim about another party as a
quotation attributed to that party with the date that party was read, says
plainly that it is not legal advice, asserts no legal conclusion, and does not
say what a score is evidence for. The architecture folder opens with how the
system is arranged rather than with the order the work happened in.

`tests/repository-sourcing.sh` reads that requirement out of the material
itself, so removing a citation, or the date beside it, turns this block red. It
proves each refusal against a fixture repository first, in both directions,
before it reads this one.

```bash
bash ../tests/repository-sourcing.sh
printf 'every external claim carries its source and the date it was read\n' | mustmatch like 'every external claim carries its source and the date it was read'
```
