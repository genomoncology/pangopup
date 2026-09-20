# Scan Rust source without trying to parse Rust expressions. The states below
# cover only tokens that can hide or introduce a string boundary: comments,
# character literals, ordinary strings, and raw strings.

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

function report(kind, line_number, detail, opening,    key) {
    key = kind SUBSEP source_path SUBSEP start_line
    if ((key in exempt_name) && opening == exempt_opening[key]) {
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
            "ordinary string contains three or more spaces between word characters",
            literal_opening == "" ? line_opening : literal_opening)
    }
}

function continuation_byte(character) {
    return character ~ /^[\200-\277]$/
}

# The harness runs Awk in the C locale. Positions are therefore bytes on both
# macOS and Linux. Return the byte width of one well-shaped UTF-8 scalar.
function utf8_width(line, position,    first, second, third, fourth) {
    first = substr(line, position, 1)
    second = substr(line, position + 1, 1)
    third = substr(line, position + 2, 1)
    fourth = substr(line, position + 3, 1)
    if (first ~ /^[\001-\177]$/) return 1
    if (first ~ /^[\302-\337]$/ && continuation_byte(second)) return 2
    if (first ~ /^[\340-\357]$/ && continuation_byte(second) && \
        continuation_byte(third)) return 3
    if (first ~ /^[\360-\364]$/ && continuation_byte(second) && \
        continuation_byte(third) && continuation_byte(fourth)) return 4
    return 0
}

# Return a closing quote position, zero for an unterminated escaped character,
# or -1 when the apostrophe begins a lifetime or label instead of a character.
function character_end(line, position,    cursor, character, width) {
    if (substr(line, position + 1, 1) == "\\") {
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

    width = utf8_width(line, position + 1)
    if (width > 0 && substr(line, position + 1 + width, 1) == "'") {
        return position + 1 + width
    }
    return -1
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
    if (raw_hashes > 255) return -1
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
    continuation_whitespace = 0
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
        key = field[2] SUBSEP field[3] SUBSEP field[4]
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
    source_bytes += length($0) + 1
    sub(/\r$/, "", line)
    line_opening = line
    work_units += length(line_opening)
    sub(/^[[:space:]]*/, "", line_opening)
    cursor = 1
    continued = 0
    if (state == "string") {
        segment_start = 1
        if (continuation_whitespace) {
            while (cursor <= length(line) && \
                substr(line, cursor, 1) ~ /^[[:space:]]$/) {
                cursor++
                work_units++
            }
            segment_start = cursor
            if (cursor > length(line)) {
                continued = 1
            } else {
                continuation_whitespace = 0
            }
        }
    }

    while (cursor <= length(line)) {
        work_units++
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
            if (character == "\"") {
                work_units += raw_closer_length
                if (substr(line, cursor, raw_closer_length) == raw_closer) {
                    cursor += raw_closer_length
                    state = "normal"
                } else {
                    cursor++
                }
            } else {
                cursor++
            }
            continue
        }

        if (state == "string") {
            if (character == "\\") {
                if (cursor == length(line)) {
                    check_collapsed(substr(line, segment_start), FNR)
                    if (literal_opening == "") literal_opening = line_opening
                    continued = 1
                    continuation_whitespace = 1
                    cursor++
                } else {
                    cursor += 2
                }
            } else if (character == "\"") {
                check_collapsed(substr(line, segment_start, cursor - segment_start), FNR)
                state = "normal"
                continuation_whitespace = 0
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

        raw_result = raw_start(line, cursor)
        if (raw_result < 0) {
            report_error(FNR, "raw string delimiter exceeds Rust's 255-hash limit")
            break
        }
        if (raw_result > 0) {
            state = "raw"
            start_line = FNR
            raw_closer = "\"" repeat("#", raw_hashes)
            raw_closer_length = length(raw_closer)
            cursor += raw_length
            continue
        }

        if ((character == "b" || character == "c") && \
            substr(line, cursor + 1, 1) == "\"") {
            state = "string"
            start_line = FNR
            literal_opening = ""
            multiline_reported = 0
            segment_start = cursor + 2
            cursor += 2
            continue
        }

        if (character == "\"") {
            state = "string"
            start_line = FNR
            literal_opening = ""
            multiline_reported = 0
            segment_start = cursor + 1
            cursor++
            continue
        }

        if (character == "'") {
            end_at = character_end(line, cursor)
            if (end_at == 0) {
                report_error(FNR, "character literal does not close on its physical line")
                break
            }
            if (end_at > 0) {
                cursor = end_at + 1
            } else {
                cursor++
            }
            continue
        }

        cursor++
    }

    if (state == "string" && !continued) {
        check_collapsed(substr(line, segment_start), FNR)
        if (!multiline_reported) {
            if (literal_opening == "") literal_opening = line_opening
            report("multiline", start_line,
                "ordinary string contains an unescaped physical newline",
                literal_opening)
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
    if (work_report != "") {
        printf "%d\t%d\n", source_bytes, work_units > work_report
        close(work_report)
    }
    if (findings > 0) exit 1
}
