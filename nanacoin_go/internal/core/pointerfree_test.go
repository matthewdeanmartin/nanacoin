package core

import (
	"reflect"
	"testing"
)

// Anything reachable from the Service that holds a pointer is a root the
// collector must trace and a candidate for the object graph we are trying not
// to have. This walks the packed structs and fails if any field is a pointer,
// slice, map, string, interface or channel.
func TestPackedStructsArePointerFree(t *testing.T) {
	for _, tc := range []struct {
		name string
		typ  reflect.Type
	}{
		{"packedUser", reflect.TypeOf(packedUser{})},
		{"packedAccount", reflect.TypeOf(packedAccount{})},
		{"packedListing", reflect.TypeOf(packedListing{})},
	} {
		t.Run(tc.name, func(t *testing.T) {
			var walk func(reflect.Type, string)
			walk = func(ty reflect.Type, path string) {
				switch ty.Kind() {
				case reflect.Ptr, reflect.Slice, reflect.Map,
					reflect.String, reflect.Interface, reflect.Chan, reflect.Func:
					t.Errorf("%s%s is %s - a pointer the GC must trace",
						tc.name, path, ty.Kind())
				case reflect.Struct:
					for i := 0; i < ty.NumField(); i++ {
						f := ty.Field(i)
						walk(f.Type, path+"."+f.Name)
					}
				case reflect.Array:
					walk(ty.Elem(), path+"[]")
				}
			}
			walk(tc.typ, "")
			t.Logf("%s: %d bytes, pointer-free", tc.name, tc.typ.Size())
		})
	}
}
