using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Text;

internal static class ManagedSurfaceProbe
{
    private static string Encode(string value)
    {
        return Convert.ToBase64String(Encoding.UTF8.GetBytes(value ?? string.Empty));
    }

    private static string Access(MethodBase method)
    {
        if (method.IsPublic) return "public";
        if (method.IsFamily || method.IsFamilyOrAssembly) return "protected";
        if (method.IsAssembly) return "assembly";
        return "private";
    }

    private static string Access(FieldInfo field)
    {
        if (field.IsPublic) return "public";
        if (field.IsFamily || field.IsFamilyOrAssembly) return "protected";
        if (field.IsAssembly) return "assembly";
        return "private";
    }

    private static void Emit(string kind, string owner, string signature, string access)
    {
        Console.WriteLine(
            string.Join("\t", kind, Encode(owner), Encode(signature), access));
    }

    public static int Main(string[] args)
    {
        if (args.Length != 1) return 2;
        var assembly = Assembly.LoadFrom(args[0]);
        var types = assembly.GetTypes().OrderBy(type => type.FullName, StringComparer.Ordinal);
        foreach (var type in types)
        {
            if (type.FullName == null || type.FullName.Contains("<")) continue;
            var typeAccess = type.IsPublic || type.IsNestedPublic
                ? "public"
                : type.IsNestedFamily || type.IsNestedFamORAssem
                    ? "protected"
                    : type.IsNotPublic || type.IsNestedAssembly
                        ? "assembly"
                        : "private";
            if (typeAccess != "private")
                Emit("type", type.FullName, type.AssemblyQualifiedName, typeAccess);

            const BindingFlags flags = BindingFlags.DeclaredOnly
                | BindingFlags.Instance | BindingFlags.Static
                | BindingFlags.Public | BindingFlags.NonPublic;
            foreach (var method in type.GetMethods(flags).Cast<MethodBase>()
                .Concat(type.GetConstructors(flags)).OrderBy(item => item.ToString()))
            {
                var access = Access(method);
                if (access != "private") Emit("method", type.FullName, method.ToString(), access);
            }
            foreach (var field in type.GetFields(flags).OrderBy(item => item.ToString()))
            {
                var access = Access(field);
                if (access != "private") Emit("field", type.FullName, field.ToString(), access);
            }
            foreach (var property in type.GetProperties(flags).OrderBy(item => item.ToString()))
            {
                var accessor = property.GetGetMethod(true) ?? property.GetSetMethod(true);
                if (accessor == null) continue;
                var access = Access(accessor);
                if (access != "private") Emit("property", type.FullName, property.ToString(), access);
            }
        }
        return 0;
    }
}
