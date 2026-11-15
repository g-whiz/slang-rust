#include <cstring>

#include "core/slang-shared-library.h"
#include "slang-glslang/slang-glslang.h"
#include "slang.h"

extern "C"
{
    int glslang_compile(glslang_CompileRequest_1_0* request);
    int glslang_compile_1_1(glslang_CompileRequest_1_1* request);
    int glslang_compile_1_2(glslang_CompileRequest_1_2* request);
    int glslang_compile_1_3(glslang_CompileRequest_1_3* request);
    bool glslang_disassembleSPIRV(const uint32_t* contents, int contentsSize);
    bool glslang_disassembleSPIRVWithResult(
        const uint32_t* contents,
        int contentsSize,
        char** outString);
    void glslang_freeDisassembly(char* disassembly);
    int glslang_linkSPIRV(glslang_LinkRequest* request);
    bool glslang_validateSPIRV(const uint32_t* contents, int contentsSize);
}

namespace
{

struct Symbol
{
    const char* name;
    void* address;
};

const Symbol symbols[] = {
    {"glslang_compile", reinterpret_cast<void*>(&glslang_compile)},
    {"glslang_compile_1_1", reinterpret_cast<void*>(&glslang_compile_1_1)},
    {"glslang_compile_1_2", reinterpret_cast<void*>(&glslang_compile_1_2)},
    {"glslang_compile_1_3", reinterpret_cast<void*>(&glslang_compile_1_3)},
    {"glslang_disassembleSPIRV", reinterpret_cast<void*>(&glslang_disassembleSPIRV)},
    {"glslang_disassembleSPIRVWithResult",
     reinterpret_cast<void*>(&glslang_disassembleSPIRVWithResult)},
    {"glslang_freeDisassembly", reinterpret_cast<void*>(&glslang_freeDisassembly)},
    {"glslang_linkSPIRV", reinterpret_cast<void*>(&glslang_linkSPIRV)},
    {"glslang_validateSPIRV", reinterpret_cast<void*>(&glslang_validateSPIRV)},
};

class LinkedLibrary final : public ISlangSharedLibrary
{
public:
    SLANG_NO_THROW SlangResult SLANG_MCALL
    queryInterface(SlangUUID const& uuid, void** out) override
    {
        void* found = castAs(uuid);
        *out = found;
        return found ? SLANG_OK : SLANG_E_NO_INTERFACE;
    }

    SLANG_NO_THROW uint32_t SLANG_MCALL addRef() override { return 1; }

    SLANG_NO_THROW uint32_t SLANG_MCALL release() override { return 1; }

    SLANG_NO_THROW void* SLANG_MCALL castAs(const SlangUUID& uuid) override
    {
        if (uuid == ISlangUnknown::getTypeGuid() || uuid == ISlangCastable::getTypeGuid() ||
            uuid == ISlangSharedLibrary::getTypeGuid())
        {
            return static_cast<ISlangSharedLibrary*>(this);
        }
        return nullptr;
    }

    SLANG_NO_THROW void* SLANG_MCALL findSymbolAddressByName(char const* name) override
    {
        for (const Symbol& symbol : symbols)
        {
            if (std::strcmp(name, symbol.name) == 0)
            {
                return symbol.address;
            }
        }
        return nullptr;
    }
};

bool namesSlangGlslang(const char* path)
{
    const char* name = path;
    for (const char* cursor = path; *cursor; ++cursor)
    {
        if (*cursor == '/' || *cursor == '\\')
        {
            name = cursor + 1;
        }
    }
    if (std::strncmp(name, "lib", 3) == 0)
    {
        name += 3;
    }
    const char prefix[] = "slang-glslang";
    return std::strncmp(name, prefix, sizeof(prefix) - 1) == 0;
}

class Loader final : public ISlangSharedLibraryLoader
{
public:
    SLANG_NO_THROW SlangResult SLANG_MCALL
    queryInterface(SlangUUID const& uuid, void** out) override
    {
        if (uuid == ISlangUnknown::getTypeGuid() ||
            uuid == ISlangSharedLibraryLoader::getTypeGuid())
        {
            *out = static_cast<ISlangSharedLibraryLoader*>(this);
            return SLANG_OK;
        }
        *out = nullptr;
        return SLANG_E_NO_INTERFACE;
    }

    SLANG_NO_THROW uint32_t SLANG_MCALL addRef() override { return 1; }

    SLANG_NO_THROW uint32_t SLANG_MCALL release() override { return 1; }

    SLANG_NO_THROW SlangResult SLANG_MCALL
    loadSharedLibrary(const char* path, ISlangSharedLibrary** out) override
    {
        if (namesSlangGlslang(path))
        {
            *out = &linkedLibrary;
            return SLANG_OK;
        }
        return Slang::DefaultSharedLibraryLoader::getSingleton()->loadSharedLibrary(path, out);
    }

private:
    LinkedLibrary linkedLibrary;
};

Loader loader;

} // namespace

extern "C" ISlangSharedLibraryLoader* slang_glslang_sys_loader(void)
{
    return &loader;
}
