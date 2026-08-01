using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// Presentation-only camera adaptation. It consumes pose data but never
    /// writes a camera result back into the Rust session.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedCameraRig : MonoBehaviour
    {
        private Camera presentationCamera;
        private UnityEnhancedCameraPose targetPose;
        private bool hasTargetPose;

        public Camera PresentationCamera { get { return presentationCamera; } }

        private void Awake()
        {
            EnsureCamera();
        }

        public void EnsureCamera()
        {
            if (presentationCamera != null)
            {
                return;
            }

            presentationCamera = GetComponentInChildren<Camera>();
            if (presentationCamera != null)
            {
                return;
            }

            GameObject cameraObject = new GameObject("EnhancedPresentationCamera");
            cameraObject.transform.SetParent(transform, false);
            presentationCamera = cameraObject.AddComponent<Camera>();
            presentationCamera.fieldOfView = 70f;
            presentationCamera.nearClipPlane = 0.05f;
            presentationCamera.farClipPlane = 2000f;
            presentationCamera.clearFlags = CameraClearFlags.SolidColor;
            presentationCamera.backgroundColor = new Color(0.015f, 0.02f, 0.04f, 1f);
        }

        public void ApplyPose(UnityEnhancedCameraPose pose)
        {
            EnsureCamera();
            presentationCamera.transform.SetPositionAndRotation(pose.Position, pose.Rotation);
            if (pose.FieldOfView > 1f && pose.FieldOfView < 179f)
            {
                presentationCamera.fieldOfView = pose.FieldOfView;
            }

            targetPose = pose;
            hasTargetPose = true;
        }

        public void SetTargetPose(UnityEnhancedCameraPose pose)
        {
            EnsureCamera();
            targetPose = pose;
            hasTargetPose = true;
        }

        public void InterpolateToTarget(float alpha)
        {
            if (!hasTargetPose)
            {
                return;
            }

            EnsureCamera();
            UnityEnhancedCameraPose current = new UnityEnhancedCameraPose(
                presentationCamera.transform.position,
                presentationCamera.transform.rotation,
                presentationCamera.fieldOfView);
            alpha = Mathf.Clamp01(alpha);
            UnityEnhancedCameraPose next = new UnityEnhancedCameraPose(
                Vector3.Lerp(current.Position, targetPose.Position, alpha),
                Quaternion.Slerp(current.Rotation, targetPose.Rotation, alpha),
                Mathf.Lerp(current.FieldOfView, targetPose.FieldOfView, alpha));
            presentationCamera.transform.SetPositionAndRotation(next.Position, next.Rotation);
            if (next.FieldOfView > 1f && next.FieldOfView < 179f)
            {
                presentationCamera.fieldOfView = next.FieldOfView;
            }
        }

        public void Interpolate(UnityEnhancedCameraPose from, UnityEnhancedCameraPose to, float alpha)
        {
            alpha = Mathf.Clamp01(alpha);
            ApplyPose(new UnityEnhancedCameraPose(
                Vector3.Lerp(from.Position, to.Position, alpha),
                Quaternion.Slerp(from.Rotation, to.Rotation, alpha),
                Mathf.Lerp(from.FieldOfView, to.FieldOfView, alpha)));
        }
    }
}
