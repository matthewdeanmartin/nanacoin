"""Encode direct C ABI fixtures as sparse, fixed-width target probe inputs."""
import json
from pathlib import Path


def emit(source: Path, destination: Path):
    data = json.loads(source.read_text(encoding="utf-8"))
    if data["abi"] != 7 or data["slots"] != 64:
        raise ValueError("Target probe requires ABI 7 / 64 slots")
    pairs, rows = [], []
    for operation, inputs, result, outputs in data["vectors"]:
        if len(inputs) > 64 or any(not 0 <= int(slot) < 64 for slot in outputs):
            raise ValueError("Fixture exceeds the public frame")
        first = len(pairs)
        pairs.extend((i, value) for i, value in enumerate(inputs) if value)
        count = len(pairs) - first
        out_first = len(pairs)
        pairs.extend((int(i), value) for i, value in outputs.items())
        rows.append((operation, result, first, count, out_first, len(pairs) - out_first))

    def literal(value):
        if not -(2**63) <= value < 2**63:
            raise ValueError("Fixture outside signed frame width")
        if value == -(2**63):
            return "INT64_MIN"
        return f"(-INT64_C({-value}))" if value < 0 else f"INT64_C({value})"

    header = '''/* Generated from public ABI boundary vectors, no Rust layouts. */
#include <stdint.h>
#include <stdio.h>
extern int NCBANK(unsigned char *, unsigned char *, unsigned char *);
struct nc_pair { unsigned slot; int64_t value; };
struct nc_vector { int32_t operation, result; unsigned first, count, out_first, out_count; };
static const struct nc_pair nc_pairs[] = {
'''
    header += "\n".join(f" {{{slot}, {literal(value)}}}," for slot, value in pairs)
    header += "\n};\nstatic const struct nc_vector nc_vectors[] = {\n"
    header += "\n".join(" {" + ",".join(map(str, row)) + "}," for row in rows)
    header += '''
};
static int nc_run_vectors(unsigned *calls) {
 for (unsigned i=0; i<sizeof(nc_vectors)/sizeof(nc_vectors[0]); ++i) {
  const struct nc_vector *v=&nc_vectors[i];
  int64_t frame[64]={0}; int32_t operation=v->operation, result=-1;
  for (unsigned j=0; j<v->count; ++j) {
   const struct nc_pair *p=&nc_pairs[v->first+j]; frame[p->slot]=p->value;
  }
  NCBANK((unsigned char *)&operation,(unsigned char *)frame,(unsigned char *)&result);
  ++*calls;
  if (result!=v->result) {
   printf("NC_VECTOR_FAIL %u op %ld status %ld expected %ld\\n",i+1,(long)operation,(long)result,(long)v->result);
   return -(int)(i+1);
  }
  for (unsigned j=0; j<v->out_count; ++j) {
   const struct nc_pair *p=&nc_pairs[v->out_first+j];
   if (frame[p->slot]!=p->value) {
    printf("NC_VECTOR_FAIL %u op %ld slot %u actual %lld expected %lld\\n",i+1,(long)operation,p->slot,(long long)frame[p->slot],(long long)p->value);
    return -(int)(i+1);
   }
  }
 }
 return 0;
}
'''
    destination.write_text(header, encoding="utf-8")
    return len(rows)
