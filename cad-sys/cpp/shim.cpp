// viewer3d CAD shim: reads STEP / IGES / BREP with OpenCASCADE and returns plain triangle meshes
// through a tiny C interface. Only standard OpenCASCADE classes are used, so the same file builds
// with MSVC, GCC and Clang, against OCCT 7.6 and newer.

#include <BRepBndLib.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BinTools.hxx>
#include <Bnd_Box.hxx>
#include <GeomLProp_SLProps.hxx>
#include <Geom_Surface.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <IGESControl_Reader.hxx>
#include <Poly_Triangle.hxx>
#include <Poly_Triangulation.hxx>
#include <STEPControl_Reader.hxx>
#include <Standard_Failure.hxx>
#include <Standard_Transient.hxx>
#include <Standard_Type.hxx>
#include <TopAbs_Orientation.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Trsf.hxx>

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <exception>
#include <map>
#include <string>
#include <vector>

// OpenCASCADE keeps every class type in one global registry, and each type removes itself from it
// when the program ends. In a statically linked program the registry can be destroyed before some
// of those types, and the program then crashes on exit (seen as 0xc0000005 / SIGSEGV after all work
// is done). Creating the registry before every other global object makes it the last one destroyed.
namespace {
struct OcctRegistryFirst {
  OcctRegistryFirst() { (void)STANDARD_TYPE(Standard_Transient); }
};
} // namespace
#if defined(_MSC_VER)
#pragma warning(disable : 4073)
#pragma init_seg(lib)
static OcctRegistryFirst g_occt_registry_first;
#elif defined(__GNUC__)
static OcctRegistryFirst g_occt_registry_first __attribute__((init_priority(101)));
#else
static OcctRegistryFirst g_occt_registry_first;
#endif

namespace {

struct Part {
  std::string name;
  std::vector<float> positions; // x y z per vertex
  std::vector<float> normals;   // x y z per vertex
  std::vector<uint32_t> indices; // 3 per triangle
};

struct Doc {
  std::vector<Part> parts;
};

void set_error(char *err, int err_len, const std::string &msg) {
  if (err == nullptr || err_len <= 0) {
    return;
  }
  std::strncpy(err, msg.c_str(), static_cast<size_t>(err_len - 1));
  err[err_len - 1] = '\0';
}

struct Key {
  float x, y, z;
  bool operator<(const Key &o) const {
    if (x != o.x) return x < o.x;
    if (y != o.y) return y < o.y;
    return z < o.z;
  }
};

// Appends the triangles of one face. Normals come from the exact surface where possible, so curved
// faces shade smoothly; they are never shared with neighbouring faces, so sharp edges stay sharp.
void add_face(const TopoDS_Face &face, Part &part) {
  TopLoc_Location loc;
  Handle(Poly_Triangulation) tri = BRep_Tool::Triangulation(face, loc);
  if (tri.IsNull() || tri->NbNodes() < 3 || tri->NbTriangles() < 1) {
    return;
  }
  const gp_Trsf trsf = loc.Transformation();
  const bool reversed = (face.Orientation() == TopAbs_REVERSED);
  const int nb_nodes = tri->NbNodes();
  const int nb_tris = tri->NbTriangles();
  const uint32_t base = static_cast<uint32_t>(part.positions.size() / 3);

  std::vector<double> p(static_cast<size_t>(nb_nodes) * 3);
  // Nodes that sit on the same spot (the seam of a cylinder, the pole of a sphere) share a normal.
  std::vector<int> group(static_cast<size_t>(nb_nodes));
  std::map<Key, int> seen;
  for (int i = 1; i <= nb_nodes; ++i) {
    gp_Pnt pt = tri->Node(i);
    pt.Transform(trsf);
    const size_t k = static_cast<size_t>(i - 1) * 3;
    p[k] = pt.X();
    p[k + 1] = pt.Y();
    p[k + 2] = pt.Z();
    const Key key = {static_cast<float>(pt.X()), static_cast<float>(pt.Y()), static_cast<float>(pt.Z())};
    std::map<Key, int>::iterator it = seen.find(key);
    if (it == seen.end()) {
      const int g = static_cast<int>(seen.size());
      seen[key] = g;
      group[static_cast<size_t>(i - 1)] = g;
    } else {
      group[static_cast<size_t>(i - 1)] = it->second;
    }
  }

  std::vector<double> acc(seen.size() * 3, 0.0);
  for (int t = 1; t <= nb_tris; ++t) {
    int a = 0, b = 0, c = 0;
    tri->Triangle(t).Get(a, b, c);
    if (a < 1 || b < 1 || c < 1 || a > nb_nodes || b > nb_nodes || c > nb_nodes) {
      continue;
    }
    if (reversed) {
      const int tmp = b;
      b = c;
      c = tmp;
    }
    const double *pa = &p[static_cast<size_t>(a - 1) * 3];
    const double *pb = &p[static_cast<size_t>(b - 1) * 3];
    const double *pc = &p[static_cast<size_t>(c - 1) * 3];
    const double ux = pb[0] - pa[0], uy = pb[1] - pa[1], uz = pb[2] - pa[2];
    const double vx = pc[0] - pa[0], vy = pc[1] - pa[1], vz = pc[2] - pa[2];
    const double nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
    const int corners[3] = {a, b, c};
    for (int j = 0; j < 3; ++j) {
      const size_t g = static_cast<size_t>(group[static_cast<size_t>(corners[j] - 1)]) * 3;
      acc[g] += nx;
      acc[g + 1] += ny;
      acc[g + 2] += nz;
    }
    part.indices.push_back(base + static_cast<uint32_t>(a - 1));
    part.indices.push_back(base + static_cast<uint32_t>(b - 1));
    part.indices.push_back(base + static_cast<uint32_t>(c - 1));
  }

  // Exact normals from the surface itself (needs the node's UV position on the surface).
  Handle(Geom_Surface) surface;
  if (tri->HasUVNodes()) {
    TopLoc_Location surface_loc;
    surface = BRep_Tool::Surface(face, surface_loc);
  }

  for (int i = 0; i < nb_nodes; ++i) {
    const size_t k = static_cast<size_t>(i) * 3;
    part.positions.push_back(static_cast<float>(p[k]));
    part.positions.push_back(static_cast<float>(p[k + 1]));
    part.positions.push_back(static_cast<float>(p[k + 2]));

    // Fallback: average of the surrounding triangles.
    const size_t g = static_cast<size_t>(group[static_cast<size_t>(i)]) * 3;
    double nx = acc[g], ny = acc[g + 1], nz = acc[g + 2];
    const double len = std::sqrt(nx * nx + ny * ny + nz * nz);
    if (len > 0.0) {
      nx /= len;
      ny /= len;
      nz /= len;
    } else {
      nx = 0.0;
      ny = 0.0;
      nz = 1.0;
    }

    if (!surface.IsNull()) {
      try {
        const gp_Pnt2d uv = tri->UVNode(i + 1);
        GeomLProp_SLProps props(surface, uv.X(), uv.Y(), 1, 1.0e-9);
        if (props.IsNormalDefined()) {
          gp_Dir d = props.Normal();
          d.Transform(trsf);
          double ex = d.X(), ey = d.Y(), ez = d.Z();
          if (reversed) {
            ex = -ex;
            ey = -ey;
            ez = -ez;
          }
          // Only trust it when it agrees with the triangles (guards against odd parametrisations).
          if (len <= 0.0 || ex * nx + ey * ny + ez * nz > 0.2) {
            nx = ex;
            ny = ey;
            nz = ez;
          }
        }
      } catch (...) {
        // keep the fallback normal
      }
    }
    part.normals.push_back(static_cast<float>(nx));
    part.normals.push_back(static_cast<float>(ny));
    part.normals.push_back(static_cast<float>(nz));
  }
}

void add_part(const TopoDS_Shape &shape, TopAbs_ShapeEnum avoid, const std::string &name, Doc &doc) {
  Part part;
  part.name = name;
  for (TopExp_Explorer ex(shape, TopAbs_FACE, avoid); ex.More(); ex.Next()) {
    add_face(TopoDS::Face(ex.Current()), part);
  }
  if (!part.indices.empty()) {
    doc.parts.push_back(part);
  }
}

// kind: 0 = STEP, 1 = IGES, 2 = BREP (text, then binary)
bool read_shape(const char *path, int kind, TopoDS_Shape &shape, std::string &error) {
  if (kind == 0) {
    STEPControl_Reader reader;
    if (reader.ReadFile(path) != IFSelect_RetDone) {
      error = "Could not read this STEP file.";
      return false;
    }
    reader.TransferRoots();
    shape = reader.OneShape();
  } else if (kind == 1) {
    IGESControl_Reader reader;
    if (reader.ReadFile(path) != IFSelect_RetDone) {
      error = "Could not read this IGES file.";
      return false;
    }
    reader.TransferRoots();
    shape = reader.OneShape();
  } else {
    BRep_Builder builder;
    bool ok = false;
    try {
      ok = BRepTools::Read(shape, path, builder);
    } catch (...) {
      ok = false;
    }
    if (!ok || shape.IsNull()) {
      TopoDS_Shape bin;
      try {
        BinTools::Read(bin, path);
      } catch (...) {
        bin.Nullify();
      }
      shape = bin;
    }
    if (shape.IsNull()) {
      error = "Could not read this BREP file.";
      return false;
    }
  }
  if (shape.IsNull()) {
    error = "The file was read, but it contains no geometry.";
    return false;
  }
  return true;
}

Doc *load_impl(const char *path, int kind, double linear_rel, double angular, std::string &error) {
  TopoDS_Shape shape;
  if (!read_shape(path, kind, shape, error)) {
    return nullptr;
  }

  // Deflection is relative to the model size, so tiny and huge models get similar detail.
  Bnd_Box box;
  BRepBndLib::Add(shape, box, Standard_False);
  double diag = 0.0;
  if (!box.IsVoid()) {
    diag = std::sqrt(box.SquareExtent());
  }
  if (!(diag > 0.0) || diag > 1.0e50) {
    diag = 100.0;
  }
  if (!(linear_rel > 0.0)) {
    linear_rel = 0.001;
  }
  if (!(angular > 0.0)) {
    angular = 0.5;
  }
  const double deflection = diag * linear_rel;
  BRepMesh_IncrementalMesh mesher(shape, deflection, Standard_False, angular, Standard_True);
  (void)mesher;

  Doc *doc = new Doc();
  int n = 0;
  for (TopExp_Explorer ex(shape, TopAbs_SOLID); ex.More(); ex.Next()) {
    ++n;
    char name[32];
    std::snprintf(name, sizeof(name), "Solid %d", n);
    add_part(ex.Current(), TopAbs_SHAPE, name, *doc);
  }
  // Faces that do not belong to any solid (surface models, open shells).
  add_part(shape, TopAbs_SOLID, n == 0 ? "Surfaces" : "Loose surfaces", *doc);

  if (doc->parts.empty()) {
    delete doc;
    error = "The CAD file contains no solid or surface geometry.";
    return nullptr;
  }
  return doc;
}

const Part *part_at(const void *handle, size_t index) {
  if (handle == nullptr) {
    return nullptr;
  }
  const Doc *doc = static_cast<const Doc *>(handle);
  if (index >= doc->parts.size()) {
    return nullptr;
  }
  return &doc->parts[index];
}

} // namespace

extern "C" {

void *vcad_load(const char *path, int kind, double linear_rel, double angular, char *err, int err_len) {
  std::string error;
  Doc *doc = nullptr;
  try {
    doc = load_impl(path, kind, linear_rel, angular, error);
  } catch (const Standard_Failure &e) {
    const char *msg = e.GetMessageString();
    error = std::string("OpenCASCADE error: ") + (msg != nullptr && msg[0] != '\0' ? msg : "unknown failure");
    doc = nullptr;
  } catch (const std::exception &e) {
    error = std::string("CAD reader error: ") + e.what();
    doc = nullptr;
  } catch (...) {
    error = "Unknown error while reading the CAD file.";
    doc = nullptr;
  }
  if (doc == nullptr) {
    set_error(err, err_len, error.empty() ? std::string("Could not read the CAD file.") : error);
  }
  return doc;
}

size_t vcad_part_count(const void *handle) {
  return handle == nullptr ? 0 : static_cast<const Doc *>(handle)->parts.size();
}

const char *vcad_part_name(const void *handle, size_t index) {
  const Part *p = part_at(handle, index);
  return p == nullptr ? "" : p->name.c_str();
}

size_t vcad_part_vertex_count(const void *handle, size_t index) {
  const Part *p = part_at(handle, index);
  return p == nullptr ? 0 : p->positions.size() / 3;
}

size_t vcad_part_index_count(const void *handle, size_t index) {
  const Part *p = part_at(handle, index);
  return p == nullptr ? 0 : p->indices.size();
}

const float *vcad_part_positions(const void *handle, size_t index) {
  const Part *p = part_at(handle, index);
  return (p == nullptr || p->positions.empty()) ? nullptr : &p->positions[0];
}

const float *vcad_part_normals(const void *handle, size_t index) {
  const Part *p = part_at(handle, index);
  return (p == nullptr || p->normals.empty()) ? nullptr : &p->normals[0];
}

const uint32_t *vcad_part_indices(const void *handle, size_t index) {
  const Part *p = part_at(handle, index);
  return (p == nullptr || p->indices.empty()) ? nullptr : &p->indices[0];
}

void vcad_free(void *handle) {
  delete static_cast<Doc *>(handle);
}

} // extern "C"
