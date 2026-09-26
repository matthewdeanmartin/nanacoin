use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn route<J: Journal>(
    s: &mut Service<J>,
    actor: MemberId,
    method: &str,
    path: &str,
    key: &str,
    body: &[u8],
    output: &mut [u8],
) -> Option<Result<usize, Error>> {
    if path != "/api/v1/commerce" && path != "/api/v1/commerce/commands" {
        return None;
    }
    Some((|| {
        if method == "GET" && path == "/api/v1/commerce" {
            return serialize(&s.state.commerce, output);
        }
        if method != "POST" || path != "/api/v1/commerce/commands" {
            return Err(Error::NotFound);
        }
        let action: crate::commerce::Action = parse(body)?;
        let receipt = s.execute_keyed(actor, key, Command::Commerce { action })?;
        serialize(&receipt, output)
    })())
}
