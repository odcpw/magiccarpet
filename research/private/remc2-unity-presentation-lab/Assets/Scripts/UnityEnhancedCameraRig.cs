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
        private UnityEnhancedCameraPose interpolatedBasePose;
        private bool hasTargetPose;
        private bool hasInterpolatedBasePose;
        private Vector3 localImpulseTranslation;
        private Quaternion localImpulseRotation = Quaternion.identity;

        public Camera PresentationCamera { get { return presentationCamera; } }
        public Vector3 InterpolatedBasePosition
        {
            get
            {
                return hasInterpolatedBasePose
                    ? interpolatedBasePose.Position
                    : (presentationCamera == null ? Vector3.zero : presentationCamera.transform.position);
            }
        }

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
            targetPose = pose;
            hasTargetPose = true;
            interpolatedBasePose = pose;
            hasInterpolatedBasePose = true;
            ApplyInterpolatedBasePose();
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
            EnsureInterpolatedBasePose();
            alpha = Mathf.Clamp01(alpha);
            interpolatedBasePose = new UnityEnhancedCameraPose(
                Vector3.Lerp(interpolatedBasePose.Position, targetPose.Position, alpha),
                Quaternion.Slerp(interpolatedBasePose.Rotation, targetPose.Rotation, alpha),
                Mathf.Lerp(interpolatedBasePose.FieldOfView, targetPose.FieldOfView, alpha));
            ApplyInterpolatedBasePose();
        }

        public void ApplyLocalImpulse(Vector3 localTranslation, Vector3 localRotationEuler)
        {
            EnsureCamera();
            EnsureInterpolatedBasePose();
            localImpulseTranslation = localTranslation;
            localImpulseRotation = Quaternion.Euler(localRotationEuler);
            ApplyInterpolatedBasePose();
        }

        public void ClearLocalImpulse()
        {
            ApplyLocalImpulse(Vector3.zero, Vector3.zero);
        }

        public void Interpolate(UnityEnhancedCameraPose from, UnityEnhancedCameraPose to, float alpha)
        {
            alpha = Mathf.Clamp01(alpha);
            ApplyPose(new UnityEnhancedCameraPose(
                Vector3.Lerp(from.Position, to.Position, alpha),
                Quaternion.Slerp(from.Rotation, to.Rotation, alpha),
                Mathf.Lerp(from.FieldOfView, to.FieldOfView, alpha)));
        }

        private void EnsureInterpolatedBasePose()
        {
            if (hasInterpolatedBasePose)
            {
                return;
            }

            interpolatedBasePose = new UnityEnhancedCameraPose(
                presentationCamera.transform.position,
                presentationCamera.transform.rotation,
                presentationCamera.fieldOfView);
            hasInterpolatedBasePose = true;
        }

        private void ApplyInterpolatedBasePose()
        {
            Vector3 position = interpolatedBasePose.Position +
                               interpolatedBasePose.Rotation * localImpulseTranslation;
            Quaternion rotation = interpolatedBasePose.Rotation * localImpulseRotation;
            presentationCamera.transform.SetPositionAndRotation(position, rotation);
            if (interpolatedBasePose.FieldOfView > 1f && interpolatedBasePose.FieldOfView < 179f)
            {
                presentationCamera.fieldOfView = interpolatedBasePose.FieldOfView;
            }
        }
    }
}
