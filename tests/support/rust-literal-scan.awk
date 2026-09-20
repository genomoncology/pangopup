# Scan Rust source without parsing Rust expressions. The harness fixes the C
# locale and each physical line is split once, so byte positions are stable on
# macOS and Linux and no per-byte operation receives the complete line.

function relative_path(filename,    prefix, path) {
    prefix = root "/"
    path = filename
    if (index(path, prefix) == 1) path = substr(path, length(prefix) + 1)
    return path
}

function report(kind, line_number, detail,    key) {
    key = kind SUBSEP source_path SUBSEP start_line
    if (key in exempt_name) {
        # Charge the one complete-opening comparison. Keep the opening global
        # so repeated findings do not pass the physical line as a scalar.
        work_units += length(exempt_opening[key])
        if ((literal_opening == "" ? line_opening : literal_opening) == \
            exempt_opening[key]) {
            exempt_used[key] = 1
            return
        }
    }
    printf "%s:%d: %s\n", source_path, line_number, detail
    findings++
}

function report_error(line_number, detail) {
    printf "%s:%d: unsupported or unterminated Rust source: %s\n", \
        source_path, line_number, detail
    findings++
}

function source_word(character) {
    return character ~ /^[[:alnum:]_]$/
}

function reset_collapsed() {
    collapsed_word = 0
    collapsed_spaces = 0
}

function remember_literal_opening() {
    if (literal_opening != "") return
    literal_opening = line_opening
    work_units += opening_length
}

# Feed one source byte from inside an ordinary literal. This preserves the old
# source-text rule, including the `n` in a written `\n` escape.
function check_collapsed_byte(character, line_number) {
    if (character == " " && collapsed_word) {
        collapsed_spaces++
        return
    }
    if (source_word(character)) {
        if (collapsed_word && collapsed_spaces >= 3) {
            report("collapsed", line_number,
                "ordinary string contains three or more spaces between word characters")
        }
        collapsed_word = 1
        collapsed_spaces = 0
        return
    }
    reset_collapsed()
}

function continuation_byte(character) {
    return character ~ /^[\200-\277]$/
}

# Positions index the current line's byte array. Return the width of one
# structurally valid UTF-8 scalar.
function utf8_width(position,    first, second, third, fourth) {
    first = byte[position]
    second = byte[position + 1]
    third = byte[position + 2]
    fourth = byte[position + 3]
    if (first ~ /^[\001-\177]$/) return 1
    if (first ~ /^[\302-\337]$/ && continuation_byte(second)) return 2
    if (first ~ /^[\340-\357]$/ && continuation_byte(second) && \
        continuation_byte(third)) return 3
    if (first ~ /^[\360-\364]$/ && continuation_byte(second) && \
        continuation_byte(third) && continuation_byte(fourth)) return 4
    return 0
}

function lifetime_start(position,    first, width) {
    first = byte[position]
    if (first ~ /^[A-Za-z_]$/) return 1
    width = utf8_width(position)
    return width > 1
}

# Return a closing quote position, zero for an unterminated character form, or
# -1 only when ALLOW_LIFETIME admits a possible lifetime or label start.
function character_end(position, allow_lifetime, ascii_only,    cursor, character, width) {
    if (byte[position + 1] == "\\") {
        cursor = position + 1
        while (cursor <= line_length) {
            work_units++
            character = byte[cursor]
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

    width = utf8_width(position + 1)
    if (width > 0 && byte[position + 1 + width] == "'") {
        if (ascii_only && width != 1) return 0
        return position + 1 + width
    }
    if (allow_lifetime && lifetime_start(position + 1)) return -1
    return 0
}

# Return 1 for an admitted raw opening, -1 for a delimiter beyond Rust's
# 255-hash limit, and zero otherwise. Set raw_length and raw_hashes on success.
function raw_start(position,    cursor, previous, prefix) {
    raw_length = 0
    raw_hashes = 0
    previous = position == 1 ? "" : byte[position - 1]
    if (previous ~ /[[:alnum:]_]/) return 0

    prefix = byte[position] byte[position + 1]
    if (prefix == "br" || prefix == "cr") {
        cursor = position + 2
    } else if (byte[position] == "r") {
        cursor = position + 1
    } else {
        return 0
    }

    while (byte[cursor] == "#") {
        raw_hashes++
        cursor++
        work_units++
    }
    if (byte[cursor] != "\"") return 0
    if (raw_hashes > 255) return -1
    raw_length = cursor - position + 1
    return 1
}

function raw_closes(position,    offset) {
    work_units += raw_hashes + 1
    for (offset = 1; offset <= raw_hashes; offset++) {
        if (byte[position + offset] != "#") return 0
    }
    return 1
}

function rust_continuation_space(character) {
    # LF is the record boundary and therefore never appears in BYTE. Empty
    # records retain continuation_whitespace below.
    return character == " " || character == "\t" || \
        character == "\r" || character == "\n"
}

function begin_source(filename) {
    source_path = relative_path(filename)
    state = "normal"
    block_depth = 0
    start_line = 0
    literal_opening = ""
    multiline_reported = 0
    continuation_whitespace = 0
    reset_collapsed()
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
    # split(..., "") is byte-oriented under the C locale fixed by the shell.
    # Charge one scan and one byte-array copy for every physical-line byte.
    record_length = split($0, byte, "")
    source_bytes += record_length + 1
    work_units += record_length * 2
    line_length = record_length
    if (line_length > 0 && byte[line_length] == "\r") line_length--

    opening_start = 1
    while (opening_start <= line_length && \
        (byte[opening_start] == " " || byte[opening_start] == "\t")) {
        opening_start++
        work_units++
    }
    opening_length = line_length - opening_start + 1
    if (opening_length > 0) {
        line_opening = substr($0, opening_start, opening_length)
        work_units += opening_length
    } else {
        line_opening = ""
    }

    cursor = 1
    continued = 0
    if (state == "string" && continuation_whitespace) {
        while (cursor <= line_length && rust_continuation_space(byte[cursor])) {
            cursor++
            work_units++
        }
        if (cursor > line_length) {
            continued = 1
        } else {
            continuation_whitespace = 0
        }
    }

    while (cursor <= line_length) {
        work_units++
        character = byte[cursor]

        if (state == "block") {
            if (character == "/" && byte[cursor + 1] == "*") {
                block_depth++
                cursor += 2
            } else if (character == "*" && byte[cursor + 1] == "/") {
                block_depth--
                cursor += 2
                if (block_depth == 0) state = "normal"
            } else {
                cursor++
            }
            continue
        }

        if (state == "raw") {
            if (character == "\"" && raw_closes(cursor)) {
                cursor += raw_hashes + 1
                state = "normal"
            } else {
                cursor++
            }
            continue
        }

        if (state == "string") {
            if (character == "\\") {
                check_collapsed_byte(character, FNR)
                if (cursor == line_length) {
                    remember_literal_opening()
                    continued = 1
                    continuation_whitespace = 1
                    cursor++
                } else {
                    check_collapsed_byte(byte[cursor + 1], FNR)
                    cursor += 2
                }
            } else if (character == "\"") {
                state = "normal"
                continuation_whitespace = 0
                reset_collapsed()
                cursor++
            } else {
                check_collapsed_byte(character, FNR)
                cursor++
            }
            continue
        }

        if (character == "/" && byte[cursor + 1] == "/") break
        if (character == "/" && byte[cursor + 1] == "*") {
            state = "block"
            block_depth = 1
            start_line = FNR
            cursor += 2
            continue
        }

        raw_result = raw_start(cursor)
        if (raw_result < 0) {
            report_error(FNR, "raw string delimiter exceeds Rust's 255-hash limit")
            break
        }
        if (raw_result > 0) {
            state = "raw"
            start_line = FNR
            cursor += raw_length
            continue
        }

        if ((character == "b" || character == "c") && \
            byte[cursor + 1] == "\"") {
            state = "string"
            start_line = FNR
            literal_opening = ""
            multiline_reported = 0
            continuation_whitespace = 0
            reset_collapsed()
            cursor += 2
            continue
        }

        if (character == "b" && byte[cursor + 1] == "'") {
            end_at = character_end(cursor + 1, 0, 1)
            if (end_at == 0) {
                report_error(FNR, "character literal does not close on its physical line")
                break
            }
            cursor = end_at + 1
            continue
        }

        if (character == "\"") {
            state = "string"
            start_line = FNR
            literal_opening = ""
            multiline_reported = 0
            continuation_whitespace = 0
            reset_collapsed()
            cursor++
            continue
        }

        if (character == "'") {
            end_at = character_end(cursor, 1, 0)
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
        reset_collapsed()
        if (!multiline_reported) {
            remember_literal_opening()
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
    if (work_report != "") {
        printf "%d\t%d\n", source_bytes, work_units > work_report
        close(work_report)
    }
    if (findings > 0) exit 1
}
