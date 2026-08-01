#if UNITY_EDITOR
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    public static class PresentationLabEditorSmoke
    {
        private const string BootScene = "Assets/Scenes/PresentationLabBoot.unity";

        public static void Run()
        {
            var scene = EditorSceneManager.OpenScene(BootScene, OpenSceneMode.Single);
            if (!scene.IsValid())
            {
                throw new BuildFailedException("presentation-lab boot scene is invalid: " + BootScene);
            }

            var bootstrap = Object.FindFirstObjectByType<PresentationLabBootstrap>();
            if (bootstrap == null)
            {
                throw new BuildFailedException("presentation-lab boot scene has no PresentationLabBootstrap");
            }

            Debug.Log("[MC2 presentation lab] editor smoke opened the owner-free boot scene");
            EditorApplication.Exit(0);
        }
    }
}
#endif
