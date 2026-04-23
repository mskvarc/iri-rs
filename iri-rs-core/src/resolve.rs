//! Relative reference resolution — oxiri port. Writes directly into output buffer.
use memchr::{memchr, memrchr};

use crate::parse::Positions;

/// Resolve `relative` against `base`, appending result to `output_buffer`.
pub fn resolve(
    base: (&str, Positions),
    relative: (&str, Positions),
    output_buffer: &mut String,
) -> Positions {
    let (base_iri, base_p) = base;
    let (rel_iri, rel_p) = relative;

    if rel_p.scheme_end != 0 {
        output_buffer.reserve_exact(rel_iri.len());
        output_buffer.push_str(rel_iri);
        return rel_p;
    }
    if rel_p.authority_end > 0 {
        output_buffer.reserve_exact(base_p.scheme_end + rel_iri.len());
        output_buffer.push_str(&base_iri[..base_p.scheme_end]);
        output_buffer.push_str(rel_iri);
        return Positions {
            scheme_end: base_p.scheme_end,
            authority_end: base_p.scheme_end + rel_p.authority_end,
            path_end: base_p.scheme_end + rel_p.path_end,
            query_end: base_p.scheme_end + rel_p.query_end,
        };
    }
    if rel_p.path_end > 0 {
        if rel_iri.starts_with('/') {
            output_buffer.reserve_exact(base_p.authority_end + rel_iri.len());
            output_buffer.push_str(&base_iri[..base_p.authority_end]);
            write_path_without_dot_segments_to(
                &rel_iri[..rel_p.path_end],
                output_buffer,
                base_p.authority_end,
                false,
            );
        } else if base_p.authority_end > base_p.scheme_end
            && base_p.authority_end == base_p.path_end
        {
            output_buffer
                .reserve_exact(base_p.authority_end + 1 + (rel_iri.len() - rel_p.authority_end));
            output_buffer.push_str(&base_iri[..base_p.authority_end]);
            write_path_without_dot_segments_to(
                &rel_iri[rel_p.authority_end..rel_p.path_end],
                output_buffer,
                base_p.authority_end,
                true,
            );
        } else if let Some(last_slash) =
            memrchr(b'/', &base_iri.as_bytes()[base_p.authority_end..base_p.path_end])
        {
            output_buffer.reserve_exact(
                base_p.authority_end + last_slash + (rel_iri.len() - rel_p.authority_end) + 1,
            );
            output_buffer.push_str(&base_iri[..base_p.authority_end]);
            if base_p.authority_end > 0 {
                write_path_without_dot_segments_to(
                    &base_iri[base_p.authority_end..][..last_slash + 1],
                    output_buffer,
                    base_p.authority_end,
                    false,
                );
                let with_prefix_slash = if output_buffer.ends_with('/') {
                    output_buffer.pop();
                    true
                } else {
                    false
                };
                write_path_without_dot_segments_to(
                    &rel_iri[rel_p.authority_end..rel_p.path_end],
                    output_buffer,
                    base_p.authority_end,
                    with_prefix_slash,
                );
            } else {
                output_buffer.push_str(&base_iri[base_p.authority_end..][..last_slash + 1]);
                output_buffer.push_str(&rel_iri[rel_p.authority_end..rel_p.path_end]);
            }
        } else {
            output_buffer
                .reserve_exact(base_p.authority_end + (rel_iri.len() - rel_p.authority_end));
            output_buffer.push_str(&base_iri[..base_p.authority_end]);
            write_path_without_dot_segments_to(
                &rel_iri[rel_p.authority_end..rel_p.path_end],
                output_buffer,
                base_p.authority_end,
                false,
            );
        }
        let path_end = output_buffer.len();
        output_buffer.push_str(&rel_iri[rel_p.path_end..]);
        return Positions {
            scheme_end: base_p.scheme_end,
            authority_end: base_p.authority_end,
            path_end,
            query_end: path_end + (rel_p.query_end - rel_p.path_end),
        };
    }
    if rel_p.query_end > 0 {
        output_buffer.reserve_exact(base_p.path_end + rel_iri.len());
        output_buffer.push_str(&base_iri[..base_p.path_end]);
        output_buffer.push_str(rel_iri);
        return Positions {
            scheme_end: base_p.scheme_end,
            authority_end: base_p.authority_end,
            path_end: base_p.path_end,
            query_end: base_p.path_end + rel_p.query_end,
        };
    }
    output_buffer.reserve_exact(base_p.query_end + rel_iri.len());
    output_buffer.push_str(&base_iri[..base_p.query_end]);
    output_buffer.push_str(rel_iri);
    base_p
}

fn write_path_without_dot_segments_to(
    mut input: &str,
    output: &mut String,
    output_path_start: usize,
    with_prefix_slash: bool,
) {
    if with_prefix_slash {
        if input.starts_with("./") {
            input = &input[1..];
        } else if input == "." {
            input = "/";
        } else if input.starts_with("../") {
            input = &input[2..];
            remove_last_segment(output, output_path_start);
        } else if input == ".." {
            input = "/";
            remove_last_segment(output, output_path_start);
        } else {
            output.push('/');
            let slash = memchr(b'/', input.as_bytes()).unwrap_or(input.len());
            output.push_str(&input[..slash]);
            input = &input[slash..];
        }
    }
    while !input.is_empty() {
        if let Some(rest) = input.strip_prefix("../") {
            input = rest;
        } else if let Some(rest) = input.strip_prefix("./") {
            input = rest;
        } else if input.starts_with("/./") {
            input = &input[2..];
        } else if input == "/." {
            input = "/";
        } else if input.starts_with("/../") {
            input = &input[3..];
            remove_last_segment(output, output_path_start);
        } else if input == "/.." {
            input = "/";
            remove_last_segment(output, output_path_start);
        } else if input == "." || input == ".." {
            input = "";
        } else {
            input = if let Some(rest) = input.strip_prefix('/') {
                output.push('/');
                rest
            } else {
                input
            };
            let slash = memchr(b'/', input.as_bytes()).unwrap_or(input.len());
            output.push_str(&input[..slash]);
            input = &input[slash..];
        }
    }
}

fn remove_last_segment(output: &mut String, output_path_start: usize) {
    let last = memrchr(b'/', &output.as_bytes()[output_path_start..]).unwrap_or(0);
    output.truncate(output_path_start + last);
}
