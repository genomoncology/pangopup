# Scan Rust source without trying to parse Rust expressions. The states below
# cover only tokens that can hide or introduce a string boundary: comments,
# character literals, ordinary strings, and raw strings.

function trim_start(value) {
    sub(/^[[:space:]]*/, "", value)
    return value
}

function repeat(character, count,    value, index_) {
    value = ""
    for (index_ = 0; index_ < count; index_++) value = value character
    return value
}

function relative_path(filename,    prefix, path) {
    prefix = root "/"
    path = filename
    if (index(path, prefix) == 1) path = substr(path, length(prefix) + 1)
    return path
}

function report(kind, line_number, detail,    key) {
    key = kind SUBSEP source_path SUBSEP start_line SUBSEP literal_opening
    if (key in exempt_name) {
        exempt_used[key] = 1
        return
    }
    printf "%s:%d: %s\n", source_path, line_number, detail
    findings++
}

function report_error(line_number, detail) {
    printf "%s:%d: unsupported or unterminated Rust source: %s\n", \
        source_path, line_number, detail
    findings++
}

function check_collapsed(segment, line_number) {
    if (segment ~ /[[:alnum:]_]   +[[:alnum:]_]/) {
        report("collapsed", line_number,
            "ordinary string contains three or more spaces between word characters")
    }
}

function probable_character(line, position,    next_character) {
    next_character = substr(line, position + 1, 1)
    if (next_character == "\\") return 1
    if (next_character !~ /[[:alnum:]_]/) return 1
    return substr(line, position + 2, 1) == "'"
}

function character_end(line, position,    cursor, character) {
    cursor = position + 1
    while (cursor <= length(line)) {
        character = substr(line, cursor, 1)
        if (character == "\\") {
            cursor += 2
        } else if (character == "'") {
            return cursor
        } else {
            cursor++
        }
    }
    return 0
}

# Set raw_length and raw_hashes when POSITION begins r"...", r#"..."#,
# br"...", br#"..."#, c r forms without the space, or their hashed forms.
function raw_start(line, position,    cursor, previous, prefix) {
    raw_length = 0
    raw_hashes = 0
    previous = position == 1 ? "" : substr(line, position - 1, 1)
    if (previous ~ /[[:alnum:]_]/) return 0

    prefix = substr(line, position, 2)
    if (prefix == "br" || prefix == "cr") {
        cursor = position + 2
    } else if (substr(line, position, 1) == "r") {
        cursor = position + 1
    } else {
        return 0
    }

    while (substr(line, cursor, 1) == "#") {
        raw_hashes++
        cursor++
    }
    if (substr(line, cursor, 1) != "\"") return 0
    raw_length = cursor - position + 1
    return 1
}

function begin_source(filename) {
    source_path = relative_path(filename)
    state = "normal"
    block_depth = 0
    start_line = 0
    literal_opening = ""
    multiline_reported = 0
}

function finish_source() {
    if (source_path == "") return
    if (state == "string") {
        report_error(start_line, "ordinary string literal reaches end of file")
    } else if (state == "raw") {
        report_error(start_line, "raw string literal reaches end of file")
    } else if (state == "block") {
        report_error(start_line, "nested block comment reaches end of file")
    }
}

BEGIN {
    source_path = ""
    while ((getline exemption < exemptions) > 0) {
        exemption_line++
        if (exemption == "" || exemption ~ /^[[:space:]]*#/) continue
        field_count = split(exemption, field, "\t")
        if (field_count != 5 || field[1] == "" || \
            (field[2] != "collapsed" && field[2] != "multiline") || \
            field[3] == "" || field[4] !~ /^[1-9][0-9]*$/ || field[5] == "") {
            printf "%s:%d: malformed Rust literal exemption\n", exemptions, exemption_line
            findings++
            continue
        }
        key = field[2] SUBSEP field[3] SUBSEP field[4] SUBSEP field[5]
        if ((key in exempt_name) || (field[1] in exempt_id)) {
            printf "%s:%d: duplicate Rust literal exemption\n", exemptions, exemption_line
            findings++
            continue
        }
        exempt_name[key] = field[1]
        exempt_id[field[1]] = 1
        exempt_path[key] = field[3]
        exempt_source_line[key] = field[4]
        exempt_opening[key] = field[5]
        exempt_line[key] = exemption_line
    }
    close(exemptions)
}

FNR == 1 {
    finish_source()
    begin_source(FILENAME)
}

{
    line = $0
    cursor = 1
    continued = 0
    if (state == "string") segment_start = 1

    while (cursor <= length(line)) {
        character = substr(line, cursor, 1)
        pair = substr(line, cursor, 2)

        if (state == "block") {
            if (pair == "/*") {
                block_depth++
                cursor += 2
            } else if (pair == "*/") {
                block_depth--
                cursor += 2
                if (block_depth == 0) state = "normal"
            } else {
                cursor++
            }
            continue
        }

        if (state == "raw") {
            remainder = substr(line, cursor)
            close_at = index(remainder, raw_closer)
            if (close_at == 0) {
                cursor = length(line) + 1
            } else {
                cursor += close_at + length(raw_closer) - 1
                state = "normal"
            }
            continue
        }

        if (state == "string") {
            if (character == "\\") {
                if (cursor == length(line)) {
                    check_collapsed(substr(line, segment_start), FNR)
                    continued = 1
                    cursor++
                } else {
                    cursor += 2
                }
            } else if (character == "\"") {
                check_collapsed(substr(line, segment_start, cursor - segment_start), FNR)
                state = "normal"
                cursor++
            } else {
                cursor++
            }
            continue
        }

        if (pair == "//") break
        if (pair == "/*") {
            state = "block"
            block_depth = 1
            start_line = FNR
            cursor += 2
            continue
        }

        if (raw_start(line, cursor)) {
            state = "raw"
            start_line = FNR
            literal_opening = trim_start(line)
            raw_closer = "\"" repeat("#", raw_hashes)
            cursor += raw_length
            continue
        }

        if ((character == "b" || character == "c") && \
            substr(line, cursor + 1, 1) == "\"") {
            state = "string"
            start_line = FNR
            literal_opening = trim_start(line)
            multiline_reported = 0
            segment_start = cursor + 2
            cursor += 2
            continue
        }

        if (character == "\"") {
            state = "string"
            start_line = FNR
            literal_opening = trim_start(line)
            multiline_reported = 0
            segment_start = cursor + 1
            cursor++
            continue
        }

        if (character == "'" && probable_character(line, cursor)) {
            end_at = character_end(line, cursor)
            if (end_at == 0) {
                report_error(FNR, "character literal does not close on its physical line")
                break
            }
            cursor = end_at + 1
            continue
        }

        cursor++
    }

    if (state == "string" && !continued) {
        check_collapsed(substr(line, segment_start), FNR)
        if (!multiline_reported) {
            report("multiline", start_line,
                "ordinary string contains an unescaped physical newline")
            multiline_reported = 1
        }
    }
}

END {
    finish_source()
    for (key in exempt_name) {
        if (!(key in exempt_used)) {
            printf "%s:%d: stale Rust literal exemption '%s' for %s:%s: %s\n", \
                exemptions, exempt_line[key], exempt_name[key], \
                exempt_path[key], exempt_source_line[key], exempt_opening[key]
            findings++
        }
    }
    if (findings > 0) exit 1
}
